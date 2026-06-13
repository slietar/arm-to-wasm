use std::{ffi::CString, marker::PhantomData};

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
    pub fn i32(&self) -> by::BinaryenType {
        unsafe { by::BinaryenInt32() }
    }

    pub fn i64(&self) -> by::BinaryenType {
        unsafe { by::BinaryenInt64() }
    }

    pub fn const_<T: ToBinaryenLiteral>(&mut self, value: T) -> Expression {
        Expression(unsafe { by::BinaryenConst(self.by_module, value.to_literal()) })
    }

    pub fn none(&self) -> by::BinaryenType {
        unsafe { by::BinaryenNone() }
    }

    pub fn relooper(&mut self) -> Relooper {
        Relooper {
            by_relooper: unsafe { by::RelooperCreate(self.by_module) },
        }
    }
}

impl Module {
    pub fn block(
        &self,
        type_: by::BinaryenType,
        expressions: &[Expression],
    ) -> Expression {
        Expression(unsafe {
            by::BinaryenBlock(
                self.by_module,
                std::ptr::null(),
                expressions.as_ptr() as *mut by::BinaryenExpressionRef,
                expressions.len() as u32,
                type_,
            )
        })
    }

    pub fn function(
        &self,
        name: &str,
        params: &[by::BinaryenType],
        result: by::BinaryenType,
        locals: &[by::BinaryenType],
        body: Expression,
    ) -> by::BinaryenFunctionRef {
        let name_cstr = CString::new(name).unwrap();

        unsafe {
            by::BinaryenAddFunction(
                self.by_module,
                name_cstr.as_ptr(),
                self.tuple(params),
                result,
                locals.as_ptr() as *mut by::BinaryenType,
                locals.len() as u32,
                body.0,
            )
        }
    }

    pub fn tuple(&self, types: &[by::BinaryenType]) -> by::BinaryenType {
        unsafe {
            by::BinaryenTypeCreate(
                types.as_ptr() as *mut by::BinaryenType,
                types.len() as u32,
            )
        }
    }

    pub fn nop(&self) -> Expression {
        Expression(unsafe { by::BinaryenNop(self.by_module) })
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

#[derive(Debug)]
#[repr(transparent)]
pub struct Expression(by::BinaryenExpressionRef);

#[derive(Debug)]
pub struct Relooper {
    by_relooper: by::RelooperRef,
}

impl Relooper {
    pub fn add_block(&mut self, code: by::BinaryenExpressionRef) -> RelooperBlock {
        RelooperBlock {
            by_block: unsafe { by::RelooperAddBlock(self.by_relooper, code) },
        }
    }

    pub fn branch(
        &self,
        from: &RelooperBlock,
        to: &RelooperBlock,
        condition: Option<by::BinaryenExpressionRef>,
    ) {
        let condition_ptr = match condition {
            Some(cond) => cond,
            None => std::ptr::null_mut(),
        };

        unsafe {
            by::RelooperAddBranch(
                from.by_block,
                to.by_block,
                condition_ptr,
                std::ptr::null_mut(),
            )
        }
    }

    pub fn finish(self, entry: &RelooperBlock) -> by::BinaryenExpressionRef {
        unsafe { by::RelooperRenderAndDispose(self.by_relooper, entry.by_block, 0) }
    }
}

#[derive(Debug)]
pub struct RelooperBlock {
    by_block: by::RelooperBlockRef,
}
