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

    pub fn optimize(&self) {
        unsafe {
            by::BinaryenModuleOptimize(self.by_module);
        }
    }

    pub fn print(&self) {
        unsafe {
            by::BinaryenModuleValidate(self.by_module);
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
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe {
            by::BinaryenModuleDispose(self.by_module);
        }
    }
}
