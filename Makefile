
all :
	wasm-pack build --debug --target no-modules
	-@rm dist
	-@mkdir dist
	cp pkg/nt4_wasm.js dist/
	cp pkg/nt4_wasm_bg.wasm dist/
	cat src/next_part.js >> dist/nt4_wasm.js
	cp static/* dist/
	
	