//! Elementi a pixel shader tenuti fra un frame e l'altro: stesso id e contatore dei
//! commit finché area e parametri non cambiano, così il damage tracker li ridisegna solo
//! quando serve.

use std::cell::RefCell;

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesPixelProgram, Uniform};
use smithay::utils::{Logical, Rectangle};

pub struct ShaderCache<K> {
    state: RefCell<Option<Cached<K>>>,
}

struct Cached<K> {
    element: PixelShaderElement,
    area: Rectangle<i32, Logical>,
    key: K,
}

impl<K> Default for ShaderCache<K> {
    fn default() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }
}

impl<K: PartialEq + Copy> ShaderCache<K> {
    /// L'elemento per quest'area e questi parametri; `uniforms` si calcola solo se cambiano.
    pub fn element(
        &self,
        program: &GlesPixelProgram,
        area: Rectangle<i32, Logical>,
        key: K,
        uniforms: impl FnOnce() -> Vec<Uniform<'static>>,
    ) -> PixelShaderElement {
        let mut state = self.state.borrow_mut();
        match state.as_mut() {
            Some(cached) if cached.area == area && cached.key == key => cached.element.clone(),
            Some(cached) => {
                cached.element.resize(area, None);
                cached.element.update_uniforms(uniforms());
                cached.area = area;
                cached.key = key;
                cached.element.clone()
            }
            None => {
                let element = PixelShaderElement::new(
                    program.clone(),
                    area,
                    None,
                    1.0,
                    uniforms(),
                    Kind::Unspecified,
                );
                *state = Some(Cached {
                    element: element.clone(),
                    area,
                    key,
                });
                element
            }
        }
    }
}
