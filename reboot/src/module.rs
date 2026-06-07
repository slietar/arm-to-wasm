use binaryen::ffi as by;

unsafe extern "C" {
    fn free(ptr: *mut std::os::raw::c_void);
}

#[derive(Debug)]
pub struct Module {
    pub by_module: by::BinaryenModuleRef,
}

impl Module {
    pub fn new() -> Self {
        let module = unsafe { by::BinaryenModuleCreate() };

        unsafe {
            by::BinaryenModuleSetFeatures(
                module,
                by::BinaryenModuleGetFeatures(module)
                    | by::BinaryenFeatureMemory64()
                    | by::BinaryenFeatureMultiMemory(),
            );
        }

        Self { by_module: module }
    }

    pub fn optimize(&mut self) {
        unsafe {
            by::BinaryenModuleOptimize(self.by_module);
        }
    }

    pub fn print(&self) {
        unsafe {
            by::BinaryenModulePrint(self.by_module);
        }
    }

    pub fn save(&self, writer: &mut impl std::io::Write) -> std::io::Result<()> {
        // let mut output = vec![0u8; 10_000_000];
        // let written = unsafe {
        //     by::BinaryenModuleWrite(self.by_module, output.as_mut_ptr() as *mut i8, output.len())
        // };

        let result =
            unsafe { by::BinaryenModuleAllocateAndWrite(self.by_module, std::ptr::null()) };

        let buffer =
            unsafe { std::slice::from_raw_parts(result.binary as *const u8, result.binaryBytes) };

        writer.write_all(buffer)?;

        unsafe {
            free(result.binary);
        }

        Ok(())
    }

    pub fn validate(&self) -> bool {
        unsafe { by::BinaryenModuleValidate(self.by_module) }
    }
}

impl Module {
    pub fn i32(&mut self) -> by::BinaryenType {
        unsafe { by::BinaryenInt32() }
    }

    pub fn i64(&mut self) -> by::BinaryenType {
        unsafe { by::BinaryenInt64() }
    }

    pub fn const_<T: ToBinaryenLiteral>(&mut self, value: T) -> by::BinaryenExpressionRef {
        unsafe { by::BinaryenConst(self.by_module, value.to_literal()) }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            by::BinaryenModuleDispose(self.by_module);
        }
    }
}

pub trait ToBinaryenLiteral {
    fn to_literal(&self) -> by::BinaryenLiteral;
}

impl ToBinaryenLiteral for i32 {
    fn to_literal(&self) -> by::BinaryenLiteral {
        unsafe { by::BinaryenLiteralInt32(*self) }
    }
}

impl ToBinaryenLiteral for i64 {
    fn to_literal(&self) -> by::BinaryenLiteral {
        unsafe { by::BinaryenLiteralInt64(*self) }
    }
}

impl ToBinaryenLiteral for f32 {
    fn to_literal(&self) -> by::BinaryenLiteral {
        unsafe { by::BinaryenLiteralFloat32(*self) }
    }
}

impl ToBinaryenLiteral for f64 {
    fn to_literal(&self) -> by::BinaryenLiteral {
        unsafe { by::BinaryenLiteralFloat64(*self) }
    }
}

impl ToBinaryenLiteral for u32 {
    fn to_literal(&self) -> by::BinaryenLiteral {
        unsafe { by::BinaryenLiteralInt32(u32::cast_signed(*self)) }
    }
}

impl ToBinaryenLiteral for u64 {
    fn to_literal(&self) -> by::BinaryenLiteral {
        unsafe { by::BinaryenLiteralInt64(u64::cast_signed(*self)) }
    }
}
