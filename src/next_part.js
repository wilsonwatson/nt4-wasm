async function run(name, ip) {
    const { _nt4_start } = wasm_bindgen;
    await wasm_bindgen('./nt4_wasm_bg.wasm');
    await _nt4_start(name, ip);
}

function watchValue(id, f) {
    const target = document.getElementById(id);
    const config = { attributes: true, childList: false, subtree: false, attributeFilter: ["value"] };
    const callback = (mutationList, _) => {
        for (const mutation of mutationList) {
            if (mutation.type === "attributes") {
                f(JSON.parse(mutation.target.getAttribute(mutation.attributeName)));
            }
        }
    }
    const observer = new MutationObserver(callback);
    observer.observe(target, config);
}