## Purpose
Use `aes` to encrypt or decrypt single files using the AES-256 algorithm in GCM (Galois Counter Mode).
You may pass in an encryption **key** to use, or provide a **salt and password** which will be used to derive a key.

A random nonce is generated for each encrypted file, and emitted as the first 12 bytes of the output.
You may pass a specific nonce to reproduce exact encrypted contents.
Do not use the same nonce for different data to encrypt. If you reuse a nonce for different content, the encryption key used can be worked out.

You can pass the Key, Salt, and Password as command line arguments (`-k`, `-s`, `-p`) or via environment variables (`AES_KEY`, `AES_SALT`, `AES_PASSWORD`). 
Keep secrets out of your command history by:
- Verify your platform keeps commands starting with [spaces out of command history](https://stackoverflow.com/questions/6475524/how-do-i-prevent-commands-from-showing-up-in-bash-history)
- Confirm by running a command starting with space, then confirming that up arrow does not show it again
- Use ` export AES_SALT=...` and ` export AES_PASSWORD=...` **with preceding spaces**
- Run `aes encrypt -i "Content.zip" -o "Content.zip.aes"`


## Installation
Select the 'release' branch in GitHub, then check the 'Releases' section on the right side for pre-built binaries for some platforms.
You may need to unblock running untrusted programs to run them.

Or,
[Install Rust](https://rust-lang.org/tools/install/) if you haven't already.
`cargo build -r` to build.


## Example Use

```
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
```