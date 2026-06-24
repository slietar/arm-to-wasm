use std::rc::Rc;

use binaryen::ffi as by;

unsafe extern "C" {
    fn free(ptr: *mut std::os::raw::c_void);
}

#[derive(Debug, Clone)]
pub struct Module {
    inner: Rc<ModuleInner>,
}

#[derive(Debug)]
pub struct ModuleInner {
    ptr: by::BinaryenModuleRef,
}

impl Drop for ModuleInner {
    fn drop(&mut self) {
        unsafe {
            by::BinaryenModuleDispose(self.ptr);
        }
    }
}

impl Module {
    pub(crate) fn module_ptr(&self) -> by::BinaryenModuleRef {
        self.inner.ptr
    }

    pub(crate) fn inner_rc(&self) -> Rc<ModuleInner> {
        self.inner.clone()
    }

    pub unsafe fn unsafe_ptr(&self) -> by::BinaryenModuleRef {
        self.inner.ptr
    }

    pub fn new() -> Self {
        let ptr = unsafe { by::BinaryenModuleCreate() };

        unsafe {
            by::BinaryenModuleSetFeatures(
                ptr,
                by::BinaryenModuleGetFeatures(ptr)
                    | by::BinaryenFeatureMemory64()
                    | by::BinaryenFeatureMultiMemory()
                    | by::BinaryenFeatureMultivalue(),
            );
        }

        Self {
            inner: Rc::new(ModuleInner { ptr }),
        }
    }

    pub fn optimize(&mut self) {
        unsafe {
            by::BinaryenModuleOptimize(self.inner.ptr);
        }
    }

    pub fn print(&self) {
        unsafe {
            by::BinaryenModulePrint(self.inner.ptr);
        }
    }

    pub fn validate(&self) -> bool {
        unsafe { by::BinaryenModuleValidate(self.inner.ptr) }
    }

    pub fn write(&self, writer: &mut impl std::io::Write) -> std::io::Result<()> {
        let result =
            unsafe { by::BinaryenModuleAllocateAndWrite(self.inner.ptr, std::ptr::null()) };

        let buffer =
            unsafe { std::slice::from_raw_parts(result.binary as *const u8, result.binaryBytes) };

        writer.write_all(buffer)?;

        unsafe {
            free(result.binary);
        }

        Ok(())
    }
}

#[test]
fn test_module() {
    let mut module = Module::new();

    assert!(module.validate());

    module.optimize();

    assert!(module.validate());

    module.function(
        "test",
        &[],
        module.none(),
        &[],
        module.nop(),
    );
}
