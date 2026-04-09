#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const { Transform, pipeline } = require('node:stream');

const USAGE = `Encrypt or decrypt single files with AES-256-GCM, using the provided password and salt.
    Copyright Scott Louvau, 2026. https://github.com/ScottLouvau/home/tree/main/tools/aes

    Examples:
      node aes.js encrypt -p "<Password>" -s "<Salt>" -i "<InFile>" -o "<OutFile>"
      node aes.js encrypt -p "<Password>" -s "<Salt>" < input.txt > output.aes

      export AES_PASSWORD=...
      export AES_SALT=...
      node obsidian-decrypt.js decrypt -i "<InFile>" -o "<OutFile>"

    Modes:
      encrypt
      decrypt
      create-salt

    Args:
      -p "<Password>", or AES_PASSWORD environment variable
      -s "<Salt>", or AES_SALT environment variable
      -k "<Key>", or AES_KEY, or derived from password and salt
      -i "<Input File>", or use stdin if not provided
      -o "<Output File>", or stdout if not provided

      --base64 to encode after encryption or decode before decryption
      --nonce "<12-byte base64 Nonce>" to use specific nonce`;

// --- Friendly Formatting ---

const SIZE_SCALES = ['bytes', 'KiB', 'MiB', 'GiB', 'TiB'];

function toFriendlySize(byteCount) {
  let size = byteCount;
  let scale = 0;
  while (size >= 1024 && scale + 1 < SIZE_SCALES.length) {
    size /= 1024;
    scale++;
  }
  const unit = SIZE_SCALES[scale];
  if (size === 0 || size >= 100) return `${size.toFixed(0)} ${unit}`;
  if (size >= 10) return `${size.toFixed(1)} ${unit}`;
  return `${size.toFixed(2)} ${unit}`;
}

function toFriendlyDuration(elapsedMs) {
  const seconds = elapsedMs / 1000;
  if (seconds < 0.01) {
    return `${(seconds * 1000).toFixed(3)} ms`;
  }
  if (seconds < 1) {
    return `${(seconds * 1000).toFixed(0)} ms`;
  }
  if (seconds < 10) {
    return `${seconds.toFixed(1)} sec`;
  }
  if (seconds < 120) {
    return `${seconds.toFixed(0)} sec`;
  }
  const minutes = seconds / 60;
  if (minutes < 10) {
    return `${minutes.toFixed(1)} min`;
  }
  if (minutes < 120) {
    return `${minutes.toFixed(0)} min`;
  }
  const hours = minutes / 60;
  if (hours < 10) {
    return `${hours.toFixed(1)} hours`;
  }
  if (hours < 48) {
    return `${hours.toFixed(0)} hours`;
  }
  const days = hours / 24;
  return `${days.toFixed(0)} days`;
}

// --- Key Derivation ---

function deriveKeyFromPassword(password, salt) {
  return crypto.scryptSync(
    Buffer.from(password.normalize('NFKC'), 'utf8'),
    Buffer.from(salt.normalize('NFKC'), 'utf8'),
    32,
    { N: 32768, r: 8, p: 1, maxmem: 128 * 32768 * 8 * 2 }
  );
}

// --- Streaming Transforms ---

class AesGcmEncryptStream extends Transform {
  constructor(key, nonce) {
    super();
    this._nonce = nonce || crypto.randomBytes(12);
    this._cipher = crypto.createCipheriv('aes-256-gcm', key, this._nonce);
    this._wroteNonce = false;
    this.bytesProcessed = 0;
  }

  _transform(chunk, encoding, callback) {
    if (!this._wroteNonce) {
      this.push(this._nonce);
      this._wroteNonce = true;
    }
    this.bytesProcessed += chunk.length;
    const encrypted = this._cipher.update(chunk);
    if (encrypted.length > 0) this.push(encrypted);
    callback();
  }

  _flush(callback) {
    if (!this._wroteNonce) {
      this.push(this._nonce);
    }
    const fin = this._cipher.final();
    if (fin.length > 0) this.push(fin);
    this.push(this._cipher.getAuthTag());
    callback();
  }
}

class AesGcmDecryptStream extends Transform {
  constructor(key, nonce) {
    super();
    this._key = key;
    this._nonce = nonce || null;
    this._decipher = null;
    this._pending = Buffer.alloc(0);
    this._nonceNeeded = nonce ? 0 : 12;
    this.bytesProcessed = 0;
  }

  _transform(chunk, encoding, callback) {
    this._pending = this._pending.length === 0
      ? chunk
      : Buffer.concat([this._pending, chunk]);

    // Extract nonce from stream head if not provided
    if (!this._decipher) {
      if (this._pending.length < this._nonceNeeded) {
        callback();
        return;
      }
      if (this._nonceNeeded > 0) {
        this._nonce = Buffer.from(this._pending.subarray(0, 12));
        this._pending = this._pending.length > 12
          ? Buffer.from(this._pending.subarray(12))
          : Buffer.alloc(0);
      }
      this._decipher = crypto.createDecipheriv('aes-256-gcm', this._key, this._nonce);
    }

    // Process all but last 16 bytes (reserved for auth tag)
    if (this._pending.length > 16) {
      const processEnd = this._pending.length - 16;
      const decrypted = this._decipher.update(this._pending.subarray(0, processEnd));
      this._pending = Buffer.from(this._pending.subarray(processEnd));
      this.bytesProcessed += decrypted.length;
      if (decrypted.length > 0) this.push(decrypted);
    }

    callback();
  }

  _flush(callback) {
    try {
      if (!this._decipher) {
        callback(new Error('Insufficient data for decryption'));
        return;
      }
      if (this._pending.length < 16) {
        callback(new Error('Incomplete data: expected at least 16-byte auth tag'));
        return;
      }
      this._decipher.setAuthTag(this._pending.subarray(this._pending.length - 16));
      if (this._pending.length > 16) {
        const decrypted = this._decipher.update(this._pending.subarray(0, this._pending.length - 16));
        this.bytesProcessed += decrypted.length;
        if (decrypted.length > 0) this.push(decrypted);
      }
      const fin = this._decipher.final();
      this.bytesProcessed += fin.length;
      if (fin.length > 0) this.push(fin);
      callback();
    } catch (err) {
      callback(new Error(`Authentication tag mismatch: ${err.message}`));
    }
  }
}

class Base64EncodeStream extends Transform {
  constructor() {
    super();
    this._remainder = Buffer.alloc(0);
  }

  _transform(chunk, encoding, callback) {
    const data = this._remainder.length > 0
      ? Buffer.concat([this._remainder, chunk])
      : chunk;
    const completeLen = Math.floor(data.length / 3) * 3;
    if (completeLen > 0) {
      this.push(data.subarray(0, completeLen).toString('base64'));
    }
    this._remainder = completeLen < data.length
      ? Buffer.from(data.subarray(completeLen))
      : Buffer.alloc(0);
    callback();
  }

  _flush(callback) {
    if (this._remainder.length > 0) {
      this.push(this._remainder.toString('base64'));
    }
    callback();
  }
}

class Base64DecodeStream extends Transform {
  constructor() {
    super();
    this._remainder = '';
  }

  _transform(chunk, encoding, callback) {
    const str = this._remainder + chunk.toString().replace(/[\s\r\n]/g, '');
    const completeLen = Math.floor(str.length / 4) * 4;
    if (completeLen > 0) {
      this.push(Buffer.from(str.substring(0, completeLen), 'base64'));
    }
    this._remainder = str.substring(completeLen);
    callback();
  }

  _flush(callback) {
    if (this._remainder.length > 0) {
      this.push(Buffer.from(this._remainder, 'base64'));
    }
    callback();
  }
}

// --- Argument Parsing ---

function parseArgs(argv, env) {
  const args = argv.slice(2);

  if (args.length < 1) {
    process.stderr.write(USAGE + '\n');
    process.exit(1);
  }

  const modeStr = args[0].toLowerCase();

  if (modeStr === 'create-salt') {
    const salt = crypto.randomBytes(32).toString('base64');
    process.stdout.write(`Generated Salt: ${salt}\n`);
    process.exit(0);
  }

  if (modeStr !== 'encrypt' && modeStr !== 'decrypt') {
    process.stderr.write(`Unknown Mode '${args[0]}'\n${USAGE}\n`);
    process.exit(2);
  }

  let password, salt, keyString, inputPath, outputPath, nonceString;
  let isBase64 = false;

  let i = 1;
  while (i < args.length) {
    const name = args[i];
    let consumed = 2;

    switch (name) {
      case '-i': inputPath = args[i + 1]; break;
      case '-o': outputPath = args[i + 1]; break;
      case '-p': password = args[i + 1]; break;
      case '-s': salt = args[i + 1]; break;
      case '-k': keyString = args[i + 1]; break;
      case '--nonce': nonceString = args[i + 1]; break;
      case '--base64': isBase64 = true; consumed = 1; break;
      default:
        process.stderr.write(`Unknown Argument '${name}'\n${USAGE}\n`);
        process.exit(2);
    }

    if (consumed === 2 && i + 1 >= args.length) {
      process.stderr.write(`Argument '${name}' missing value.\n${USAGE}\n`);
      process.exit(2);
    }

    i += consumed;
  }

  // Fallback to environment variables
  if (password === undefined && env.AES_PASSWORD) password = env.AES_PASSWORD;
  if (salt === undefined && env.AES_SALT) salt = env.AES_SALT;
  if (keyString === undefined && env.AES_KEY) keyString = env.AES_KEY;

  // Decode nonce if provided
  let nonce = null;
  if (nonceString !== undefined) {
    nonce = Buffer.from(nonceString, 'base64');
    if (nonce.length !== 12) {
      process.stderr.write(`Nonce, "${nonceString}", was not 12 bytes when decoded.\n`);
      process.exit(2);
    }
  }

  // Derive or decode key
  let key;
  if (keyString !== undefined) {
    key = Buffer.from(keyString, 'base64');
    if (key.length !== 32) {
      process.stderr.write('Key was not 32 bytes.\n');
      process.exit(2);
    }
  } else if (password !== undefined && salt !== undefined) {
    key = deriveKeyFromPassword(password, salt);
  } else {
    process.stderr.write('Requires key, or password and salt, either via arguments or in environment variables AES_KEY, AES_PASSWORD, AES_SALT.\n');
    process.exit(3);
  }

  return { mode: modeStr, key, nonce, inputPath, outputPath, isBase64 };
}

// --- Main ---

function main() {
  const { mode, key, nonce, inputPath, outputPath, isBase64 } = parseArgs(process.argv, process.env);

  const input = inputPath
    ? fs.createReadStream(inputPath)
    : process.stdin;

  const output = outputPath
    ? fs.createWriteStream(outputPath)
    : process.stdout;

  const streams = [input];

  if (mode === 'decrypt' && isBase64) {
    streams.push(new Base64DecodeStream());
  }

  const cryptoStream = mode === 'encrypt'
    ? new AesGcmEncryptStream(key, nonce)
    : new AesGcmDecryptStream(key, nonce);
  streams.push(cryptoStream);

  if (mode === 'encrypt' && isBase64) {
    streams.push(new Base64EncodeStream());
  }

  streams.push(output);

  const start = performance.now();

  pipeline(...streams, (err) => {
    if (err) {
      process.stderr.write(`Error: ${err.message}\n`);
      process.exit(1);
    }
    const elapsed = performance.now() - start;
    process.stderr.write(`Done. ${toFriendlySize(cryptoStream.bytesProcessed)} processed in ${toFriendlyDuration(elapsed)}\n`);
  });
}

main();
