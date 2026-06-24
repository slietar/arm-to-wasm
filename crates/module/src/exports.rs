use binaryen::ffi as by;
use std::ffi::CString;

use crate::{Module, Type};

impl Module {
    pub fn import_function(
        &self,
        internal_name: &str,
        external_module_name: &str,
        external_function_name: &str,
        params: &[Type],
        result: Type,
    ) {
        let internal_name_cstr = CString::new(internal_name).unwrap();
        let external_module_name_cstr = CString::new(external_module_name).unwrap();
        let external_function_name_cstr = CString::new(external_function_name).unwrap();

        unsafe {
            by::BinaryenAddFunctionImport(
                self.module_ptr(),
                internal_name_cstr.as_ptr(),
                external_module_name_cstr.as_ptr(),
                external_function_name_cstr.as_ptr(),
                self.tuple_type(params).ptr,
                result.ptr,
            )
        }
    }

    pub fn export_function(&self, internal_name: &str, external_name: &str) {
        let internal_name_cstr = CString::new(internal_name).unwrap();
        let external_name_cstr = CString::new(external_name).unwrap();

        unsafe {
            by::BinaryenAddFunctionExport(
                self.module_ptr(),
                internal_name_cstr.as_ptr(),
                external_name_cstr.as_ptr(),
            );
        }
    }
}
