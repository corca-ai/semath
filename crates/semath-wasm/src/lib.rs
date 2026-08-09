use semath_core::SemathEngine as CoreEngine;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Default)]
pub struct SemathEngine {
    core: CoreEngine,
}

#[wasm_bindgen(js_name = inspectPackCatalog)]
pub fn inspect_pack_catalog(payload: &[u8]) -> Result<Vec<u8>, JsError> {
    semath_core::inspect_pack_catalog_json(payload)
        .map_err(|error| JsError::new(&error.to_string()))
}

#[wasm_bindgen(js_name = createPackTemplate)]
pub fn create_pack_template(pack_id: &str) -> Result<String, JsError> {
    semath_core::pack_template(pack_id).map_err(|error| JsError::new(&error.to_string()))
}

#[wasm_bindgen]
impl SemathEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    #[wasm_bindgen(js_name = resetProject)]
    pub fn reset_project(&mut self, payload: &[u8]) -> Result<Vec<u8>, JsError> {
        self.core
            .reset_json(payload)
            .map_err(|error| JsError::new(&error.to_string()))
    }

    #[wasm_bindgen(js_name = applyChanges)]
    pub fn apply_changes(&mut self, payload: &[u8]) -> Result<Vec<u8>, JsError> {
        self.core
            .apply_json(payload)
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub fn query(&self, payload: &[u8]) -> Result<Vec<u8>, JsError> {
        self.core
            .query_json(payload)
            .map_err(|error| JsError::new(&error.to_string()))
    }
}
