echo "Building WASM..."
wasm-pack build --target web --out-dir pkg
echo "Running server..."
python3 -m http.server 8000