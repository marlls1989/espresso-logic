// Wrapper to properly load and initialize the Emscripten WASM module

let wasmModule = null;
let initPromise = null;

export async function initWasm() {
  if (wasmModule) {
    return wasmModule;
  }

  if (!initPromise) {
    initPromise = new Promise((resolve, reject) => {
      // Create script element to load Emscripten generated JS
      const script = document.createElement('script');
      script.src = '/espresso_demo.js';
      script.async = true;
      
      script.onload = () => {
        // Wait for Module to be available and initialized
        const checkInterval = setInterval(() => {
          if (window.Module && window.Module._minimise_expressions) {
            clearInterval(checkInterval);
            wasmModule = window.Module;
            console.log('WASM module initialized successfully');
            resolve(wasmModule);
          }
        }, 100);
        
        // Timeout after 10 seconds
        setTimeout(() => {
          if (!wasmModule) {
            clearInterval(checkInterval);
            reject(new Error('WASM module initialization timeout'));
          }
        }, 10000);
      };
      
      script.onerror = () => {
        reject(new Error('Failed to load WASM module script'));
      };
      
      document.head.appendChild(script);
    });
  }

  return initPromise;
}

export function callMinimise(inputText, coverType) {
  if (!wasmModule) {
    throw new Error('WASM module not initialized');
  }

  try {
    // Call the function using ccall
    const resultPtr = wasmModule.ccall(
      'minimise_expressions',
      'number',
      ['string', 'number'],
      [inputText, coverType]
    );
    
    // Read the result
    const resultJson = wasmModule.UTF8ToString(resultPtr);
    
    // Free the result string
    wasmModule.ccall('free_string', null, ['number'], [resultPtr]);
    
    // Parse and return
    return JSON.parse(resultJson);
  } catch (error) {
    console.error('Error calling WASM function:', error);
    throw error;
  }
}

