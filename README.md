### AES
See README in https://github.com/ScottLouvau/rust_tools/tree/main/aes.

Encrypt or Decrypt single files using AES-256 in GCM (Galois Counter Mode).
Generates a random nonce for each file.
Uses a salt and password to derive the encryption key used.


### Episode Renamer
See README in https://github.com/ScottLouvau/rust_tools/tree/main/episode-renamer.

Rename a set of files given:
- A TSV with one row per file showing name mappings
- A format string showing the current file name format
- A format string showing the desired file name format


### To Release
Releases are automatically built from tagged pushes in the Release branch.

```
git checkout release
git merge main
git tag v1.0.001
git push origin v1.0.001
```
