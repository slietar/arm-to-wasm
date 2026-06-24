use binaryen::ffi as by;

use crate::core::Module;

#[derive(Debug, Clone)]
pub struct Type {
    pub(crate) ptr: by::BinaryenType,
}

impl Module {
    pub fn i32(&self) -> Type {
        Type {
            ptr: unsafe { by::BinaryenInt32() },
        }
    }

    pub fn i64(&self) -> Type {
        Type {
            ptr: unsafe { by::BinaryenInt64() },
        }
    }

    pub fn none(&self) -> Type {
        Type {
            ptr: unsafe { by::BinaryenNone() },
        }
    }

    pub fn tuple_type(&self, types: &[Type]) -> Type {
        let mut type_ptrs: Vec<_> = types.iter().map(|t| t.ptr).collect();

        Type {
            ptr: unsafe {
                by::BinaryenTypeCreate(type_ptrs.as_mut_ptr(), type_ptrs.len() as u32)
            },
        }
    }
}
