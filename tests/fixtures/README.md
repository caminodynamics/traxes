`file_write_v1.json` freezes the pre-outcome FILE_WRITE artifact schema and
fingerprint composition from commit `3fe51c2` (PR #3's base). It uses synthetic,
portable inputs and deliberately omits `execution_outcome`.

The exact original v1 hash input is checked in as `file_write_v1.preimage.json`;
its SHA-256 was computed independently of the current artifact builder.
`file_write_v1.yaml` supplies the matching policy. Keep these fixtures frozen:
rebuilding them with the current writer would hide compatibility regressions.
