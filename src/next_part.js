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

function watchTree(id, add, sub) {
    const target = document.getElementById(id);
    const config = { attributes: false, childList: true, subtree: false };
    const callback = (mutationList, _) => {
        for (const mutation of mutationList) {
            if(mutation.type === "childList") {
                if(add) {
                    for(const added of mutation.addedNodes) {
                        add(added)
                    }
                }
                if(sub) {
                    for(const subtracted of mutation.removedNodes) {
                        sub(subtracted)
                    }
                }
            }
        }
    }
    const observer = new MutationObserver(callback);
    observer.observe(target, config);
}