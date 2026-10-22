#!/usr/bin/env bash
# Downloads Microsoft's official ONNX Runtime build for Apple silicon into
# vendor/onnxruntime/ (git-ignored) and verifies its SHA-256. ob-ml loads it at
# run time to run the neural model on the GPU and Neural Engine via CoreML.
# The version matches the one the `ort` crate (2.0.0-rc.12) is built against.
set -euo pipefail

cd "$(dirname "$0")/../.."
VERSION="1.24.2"
NAME="onnxruntime-osx-arm64-${VERSION}"
SHA="0af4fa503e8ea285245b47ee42d0a7461b8156a81270857da0c1d4ecf858abde"
DEST="vendor/onnxruntime"
DYLIB="${DEST}/${NAME}/lib/libonnxruntime.${VERSION}.dylib"

if [ -f "${DYLIB}" ]; then
    echo "${DYLIB}: present"
    exit 0
fi
mkdir -p "${DEST}"
curl -fL --retry 3 -o "${DEST}/${NAME}.tgz.part" "https://github.com/microsoft/onnxruntime/releases/download/v${VERSION}/${NAME}.tgz"
if ! echo "${SHA}  ${DEST}/${NAME}.tgz.part" | shasum -a 256 -c --status; then
    rm -f "${DEST}/${NAME}.tgz.part"
    echo "checksum mismatch, removed" >&2
    exit 1
fi
tar -xzf "${DEST}/${NAME}.tgz.part" -C "${DEST}"
rm -f "${DEST}/${NAME}.tgz.part"
echo "${DYLIB}: downloaded and verified"
