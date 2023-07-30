async function run(name, ip) {
    const { _nt4_start } = wasm_bindgen;
    await wasm_bindgen('./nt4_wasm_bg.wasm');
    await _nt4_start(name, ip);
}