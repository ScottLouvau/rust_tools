use aes_gcm::{AeadCore, Aes256Gcm, aead::{KeyInit, OsRng}};
use aes_gcm_stream::{Aes256GcmStreamDecryptor, Aes256GcmStreamEncryptor};
use base64::{Engine, prelude::BASE64_STANDARD};
use base64::read::DecoderReader as Base64DecoderReader;
use base64::write::EncoderWriter as Base64EncoderWriter;
use scrypt::{password_hash::SaltString, scrypt};
use std::{collections::HashMap, fs::File, io::Cursor, time::Instant};
use std::io::{Read, Write};

mod friendly;

// https://obsidian.md/blog/verify-obsidian-sync-encryption/

// Buffer Size for streaming encrypt/decrypt
const BUFFER_SIZE: usize = 64 * 1024; 

const USAGE: &str = r#"Encrypt or decrypt single files with AES-256-GCM, using the provided password and salt.
    Copyright Scott Louvau, 2026. https://github.com/ScottLouvau/home/tree/main/tools/aes
    
    Examples:
      aes encrypt -p "<Password>" -s "<Salt>" -i "<InFile>" -o "<OutFile>"
      aes encrypt -p "<Password>" -s "<Salt>" < input.txt > output.aes

      export AES_PASSWORD=...
      export AES_SALT=...
      aes decrypt -i "<InFile>" -o "<OutFile>"

    Modes:
      aes encrypt
      aes decrypt
      aes create-salt

    Args:
      -p "<Password>", or AES_PASSWORD environment variable
      -s "<Salt>", or AES_SALT environment variable
      -k "<Key>", or AES_KEY, or derived from password and salt
      -i "<Input File>", or use stdin if not provided
      -o "<Output File>", or stdout if input is stdin, or default to "<InputFile>.aes"

      --base64 to encode after encryption or decode before decryption
      --nonce "<12-byte base64 Nonce>" to use specific nonce
"#;

/*
  Features:
    - Encrypt given password and salt.
        - Streaming, from stdin to stdout.
        - From file to output file (w/default output file name pattern).
        - Password and salt can be args or environment variables.
        - Key may be provided instead of password and salt.
        - Nonce may be provided, and is otherwise generated.
        - Output file 'aes256', then nonce, then ciphertext.
    - Decrypt given password and salt.
        - Same as encrypt: streaming or files, args or environment variables.        
        - Warn but tolerate for input without 'aes256' prefix.
        - Nonce may be provided, and then not expected as prefix in file.
        - Clear errors for:
           - Can't find or read file.
           - Salt length? (Need to see restrictions)
           - Nonce length, if passed.

    - Generate a salt as base64 and output
    - Option for base64 encode/decode? (Handle Obsidian case fully)

    - V2: Support encrypting or decrypting a directory, using zstd compression. 
 */

#[derive(Debug)]
pub enum AesError {
    Io(std::io::Error),
    Aes(aes_gcm::Error),
    Base64(base64::DecodeError)
}

impl From<std::io::Error> for AesError {
    fn from(e: std::io::Error) -> Self {
        AesError::Io(e)
    }
}

impl From<aes_gcm::Error> for AesError {
    fn from(e: aes_gcm::Error) -> Self {
        AesError::Aes(e)
    }
}

enum Mode {
    Encrypt,
    Decrypt
}

struct Args {
    mode: Mode,
    key: [u8; 32],
    nonce: Option<[u8; 12]>,
    input: Box<dyn Read>,
    output: Box<dyn Write>
}

impl Args {
    fn from_arguments(args: &[String], vars: &HashMap<String, String>) -> Result<Args, (String, i32)> {
        // Skip program name
        let mut args = &args[1..];

        let mode = args[0].to_ascii_lowercase();
        let mode = match mode.as_ref() {
            "encrypt" => Mode::Encrypt,
            "decrypt" => Mode::Decrypt,
            _ => {
                return Err((format!("Unknown Mode '{mode}'\n{USAGE}"), 2));
            }
        };

        // Skip mode
        args = &args[1..]; 

        let mut password = None;
        let mut salt = None;
        let mut key_string = None;
        let mut input_file_path = None;
        let mut output_file_path = None;
        let mut nonce_string = None;
        let mut is_base64 = false;

        // Parse command line arguments
        while args.len() > 0 {
            let name = args[0].as_str();
            let value = args.get(1);
            let mut consumed_count = 2;

            match name {
                "-i" => { input_file_path = value.map(|s| s.to_string()); }
                "-o" => { output_file_path = value.map(|s| s.to_string()); }
                "-p" => { password = value.map(|s| s.to_string()); }
                "-s" => { salt = value.map(|s| s.to_string()); }
                "-k" => { key_string = value.map(|s| s.to_string()); }
                "--nonce" => { nonce_string = value.map(|s| s.to_string()); }
                "--base64" => { is_base64 = true; consumed_count = 1; }
                _ => {
                    return Err((format!("Unknown Argument '{name}'\n{USAGE}"), 2));
                }
            }

            // Error if an argument was missing the value
            if args.len() < consumed_count {
                return Err((format!("Argument '{name}' missing value.\n{USAGE}"), 2));
            }

            args = &args[consumed_count..];
        }

        // Load arguments from environment variables, if not found in arguments
        if password.is_none() {
            if let Some(password_var) = vars.get("AES_PASSWORD") {
                password = Some(password_var.clone());
            }
        }

        if salt.is_none() {
            if let Some(salt_var) = vars.get("AES_SALT") {
                salt = Some(salt_var.clone());
            }
        }

        if key_string.is_none() {
            if let Some(key_var) = vars.get("AES_KEY") {
                key_string = Some(key_var.clone());
            }
        }

        // Decode nonce if provided
        let nonce_bytes: [u8; 12];
        let mut nonce: Option<[u8; 12]> = None;
        if let Some(nonce_string) = &nonce_string {
            let nonce_vec = match BASE64_STANDARD.decode(nonce_string) {
                Ok(v) => v,
                Err(_) => {
                    return Err((format!("Could not base64 decode provided nonce, \"{nonce_string}\""), 3));
                }
            };

            nonce_bytes = match nonce_vec.try_into() {
                Ok(b) => b,
                Err(_) => {
                    return Err((format!("Nonce, \"{nonce_string}\", was not 12 bytes when decoded."), 2));
                }
            };

            nonce = Some(nonce_bytes);
        }

        // Decode or Derive key; error if neither key nor (password + salt) was provided
        let key_vec;
        if let Some(key_string) = &key_string {
            key_vec = match BASE64_STANDARD.decode(key_string) {
                Ok(v) => v,
                Err(_) => {
                    return Err((format!("Unable to base64 decode provided key, \"{key_string}\"."), 3));
                }
            };            
        } else {
            if let Some(password) = &password {
                if let Some(salt) = &salt {
                    key_vec = match derive_key_from_password(password, salt) {
                        Ok(key) => key,
                        Err(e) => {
                            return Err((format!("Unable to generate key from password \"{password}\" and salt \"{salt}\": {e:?}"), 3));
                        }
                    };
                } else {
                    return Err((format!("Requires key, or password and salt, either via arguments or in environment variables AES_KEY, AES_PASSWORD, AES_SALT."), 3));
                }
            } else {
                return Err((format!("Requires key, or password and salt, either via arguments or in environment variables AES_KEY, AES_PASSWORD, AES_SALT."), 3));
            }
        }

        // Convert key to fixed length array and validate length
        let key: [u8; 32];
        key = match key_vec.try_into() {
            Ok(b) => b,
            Err(_) => {
                return Err((format!("Key was not 32 bytes."), 2));
            }
        };

        // Setup input and output streams
        let mut input: Box<dyn Read> = match &input_file_path {
            Some(path) => Box::new(File::open(path).expect(&format!("Error opening input file \"{path}\"."))),
            None => Box::new(std::io::stdin())
        };

        let mut output: Box<dyn Write> = match &output_file_path {
            Some(path) => Box::new(File::create(&path).expect(&format!("Error creating output file \"{path}\"."))),
            None => Box::new(std::io::stdout())
        };

        if is_base64 {
            match mode {
                Mode::Encrypt => {
                    // For base64 mode, we encode in the writer before writing
                    output = Box::new(Base64EncoderWriter::new(output, &BASE64_STANDARD));
                },
                Mode::Decrypt => {
                    // For base64 mode, we decode in the reader before returning
                    input = Box::new(Base64DecoderReader::new(input, &BASE64_STANDARD));
                }
            }
        }

        Ok(Args {
            mode: mode,
            key: key,
            nonce: nonce,
            input: input,
            output: output
        })
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let vars: HashMap<String, String> = std::env::vars().collect();

    if args.len() < 2 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let mode = args[1].to_ascii_lowercase();
    if mode == "create-salt" {
        let salt = generate_random_salt();
        let salt = BASE64_STANDARD.encode(&salt);
        println!("Generated Salt: {salt}");
        std::process::exit(0);
    }

    let mut args = match Args::from_arguments(&args, &vars) {
        Ok(a) => a,
        Err((message, code)) => {
            eprintln!("{}", message);
            std::process::exit(code);
        }
    };

    let start = Instant::now();
    let bytes_processed: usize;
    match args.mode {
        Mode::Encrypt => {
            bytes_processed = encrypt_stream(args.key, args.nonce, args.input, &mut args.output).expect("Error encrypting content.");
        },
        Mode::Decrypt => {            
            bytes_processed = decrypt_stream(args.key, args.nonce, args.input, &mut args.output).expect("Error decrypting or validating data.");
        }
    }

    let runtime = Instant::now() - start;
    eprintln!("Done. {} processed in {}", friendly::to_friendly_size(bytes_processed as u64), friendly::to_friendly_duration(runtime));
}

fn encrypt_stream<R: Read, W: Write>(
    key: [u8; 32],
    nonce: Option<[u8; 12]>,
    mut reader: R,
    mut writer: W,
) -> Result<usize, AesError> {
    // Use provided nonce or generate a random one; always 12 bytes for AES-GCM
    let nonce: [u8; 12] = match nonce {
        Some(n) => n,
        None => Aes256Gcm::generate_nonce(&mut OsRng).into(),
    };

    // Write nonce as a prefix so decrypt_stream can recover it
    writer.write_all(&nonce)?;

    let mut encryptor = Aes256GcmStreamEncryptor::new(key, &nonce);

    let mut bytes_encrypted = 0;
    let mut buf = [0u8; BUFFER_SIZE];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        bytes_encrypted += n;
        writer.write_all(&encryptor.update(&buf[..n]))?;
    }

    let (last_block, tag) = encryptor.finalize();
    writer.write_all(&last_block)?;
    writer.write_all(&tag)?;
    writer.flush()?;

    Ok(bytes_encrypted)
}

fn decrypt_stream<R: Read, W: Write>(
    key: [u8; 32],
    nonce: Option<[u8; 12]>,
    mut reader: R,
    mut writer: W,
) -> Result<usize, AesError> {
    // If a nonce was supplied, use it directly; otherwise read the 12-byte prefix
    let nonce: [u8; 12] = match nonce {
        Some(n) => n,
        None => {
            let mut n = [0u8; 12];
            reader.read_exact(&mut n)?;
            n
        }
    };

    let mut decryptor = Aes256GcmStreamDecryptor::new(key, &nonce);

    let mut bytes_decrypted = 0;
    let mut buf = [0u8; BUFFER_SIZE];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        let plaintext = decryptor.update(&buf[..n]);
        bytes_decrypted += plaintext.len();
        writer.write_all(&plaintext)?;
    }

    let last_plaintext = decryptor.finalize().map_err(|e| AesError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("Authentication tag mismatch: {e}")
    )))?;
    bytes_decrypted += last_plaintext.len();
    writer.write_all(&last_plaintext)?;
    writer.flush()?;

    Ok(bytes_decrypted)
}

pub fn decrypt(key: [u8; 32], data: &[u8]) -> Result<Vec<u8>, AesError> {
    let mut result = Vec::new();
    decrypt_stream(key, None, &mut Cursor::new(data), &mut result)?;
    Ok(result)
}

pub fn encrypt(key: [u8; 32], data: &[u8], nonce: Option<[u8; 12]>) -> Result<Vec<u8>, AesError> {
    let mut result = Vec::new();
    encrypt_stream(key, nonce, &mut Cursor::new(data), &mut result)?;    
    Ok(result)
}

pub fn derive_key_from_password(password: &str, salt: &str) -> Result<Vec<u8>, AesError> {
    let password = password.as_bytes();
    let salt = salt.as_bytes();
    let params = scrypt::Params::new(15, 8, 1, scrypt::Params::RECOMMENDED_LEN).expect("Invalid scrypt parameters");
    let mut key = [0u8; scrypt::Params::RECOMMENDED_LEN];
    scrypt(password, salt, &params, &mut key).expect("Couldn't hash password");
    Ok(key.to_vec())
}

// Generate a random nonce (12 bytes for AES-GCM)
pub fn generate_nonce() -> [u8; 12] {
    Aes256Gcm::generate_nonce(&mut OsRng).try_into().expect("Generated nonce wasn't 12 bytes")
}

// Generate a random salt for scrypt encryption
pub fn generate_random_salt() -> String {
    SaltString::generate(&mut OsRng).to_string()
}

// Generate a random AES-256 key (32 bytes = 256 bits)
pub fn generate_key() -> [u8; 32] {
    Aes256Gcm::generate_key(OsRng).try_into().expect("Generated key wasn't 32 bytes.")
}

// pub fn friendly_time(d: Duration) -> String {
//     let seconds = d.as_secs_f64();

//     if seconds > (2 * 60 * 60) as f64 {

//     }
// }


/* 
  Rust using 'nonce' term vs 'IV' (same thing; 12-byte = 96 bit)
  Obsidian generates a salt per vault.
  Obsidian uses scrypt to derive the key from the password and salt, with parameters N=32768, r=8, p=1, maxmem=128*32768*8*2
 */

 #[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE_PASSWORD: &str = "sample password";
    const SAMPLE_SALT: &str = "8zUqk?w*rnU7LneIzJR&";
    const SAMPLE_KEY_BASE64: &str = "mf+ZT+unDv+jLz1ywmpLRozL6AXJWRGhPoNunte12fo=";

    #[test]
    fn test_encryption_decryption() {
        // Verify successful encrypt/decrypt roundtrip
        let key = generate_key();
        let plaintext = b"Hello, World!";
        let nonce = generate_nonce();

        let ciphertext = encrypt(key, plaintext, Some(nonce)).unwrap();
        let decrypted = decrypt(key, &ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_derive_key_from_password() {
        // Verify scrypt key derivation matches Node.js crypto.scryptSync with the same parameters
        let key = derive_key_from_password(SAMPLE_PASSWORD, SAMPLE_SALT).unwrap();
        let expected = BASE64_STANDARD.decode(SAMPLE_KEY_BASE64).unwrap();

        assert_eq!(key, expected);
    }

    #[test]
    fn test_parse_args() {
        let mut env_vars = HashMap::new();
        let expected_key: [u8; 32] = BASE64_STANDARD.decode(SAMPLE_KEY_BASE64).unwrap().try_into().unwrap();

        // Verify password and salt via arguments result in scrypt key derivation and expected key
        let args: Vec<String> = vec!["aes", "encrypt", "-p", "sample password", "-s", "8zUqk?w*rnU7LneIzJR&"].into_iter().map(String::from).collect();      
        let result = Args::from_arguments(&args, &env_vars).expect("Arguments should parse successfully");
        assert_eq!(result.key, expected_key);

        // Verify key can be passed directly
        let args: Vec<String> = vec!["aes", "encrypt", "-k", SAMPLE_KEY_BASE64].into_iter().map(String::from).collect();      
        let result = Args::from_arguments(&args, &env_vars).expect("Arguments should parse successfully");
        assert_eq!(result.key, expected_key);

        // Verify key can be in environment args only
        env_vars.insert("AES_KEY".into(), SAMPLE_KEY_BASE64.into());
        let args: Vec<String> = vec!["aes", "encrypt"].into_iter().map(String::from).collect();      
        let result = Args::from_arguments(&args, &env_vars).expect("Arguments should parse successfully");
        assert_eq!(result.key, expected_key);

        // Verify args don't parse if password and salt aren't both provided
        env_vars.clear();
        env_vars.insert("AES_PASSWORD".into(), SAMPLE_PASSWORD.into());
        let args: Vec<String> = vec!["aes", "encrypt"].into_iter().map(String::from).collect();      
        let result = Args::from_arguments(&args, &env_vars);
        assert!(result.is_err());

        // Verify successful with salt in args and password in environment 
        let args: Vec<String> = vec!["aes", "encrypt", "-s", SAMPLE_SALT].into_iter().map(String::from).collect();      
        let result = Args::from_arguments(&args, &env_vars).expect("Arguments should parse successfully");
        assert_eq!(result.key, expected_key);
    }
}