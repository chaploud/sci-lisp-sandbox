RELEASE="--release"

if [ "$1" = "debug" ]; then
  RELEASE=""
fi

TARGET_DIR=$(pwd)/target

cd src/core
CARGO_TARGET_DIR=${TARGET_DIR} cargo build --target wasm32-wasi ${RELEASE}

cd ../../
cargo build ${RELEASE}
