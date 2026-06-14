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
                    | by::BinaryenFeatureMultiMemory()
                    | by::BinaryenFeatureMultivalue(),
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

    pub fn const_<T: ToBinaryenLiteral>(&self, value: T) -> Expression {
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
    pub fn binary(&self, operand1: Expression, operand2: Expression, op: BinaryOp) -> Expression {
        Expression(unsafe { by::BinaryenBinary(self.by_module, op.to_binaryen_op(), operand1.0, operand2.0) })
    }

    pub fn drop(&mut self, expr: Expression) -> Expression {
        Expression(unsafe { by::BinaryenDrop(self.by_module, expr.0) })
    }

    pub fn unary(&self, operand: Expression, op: UnaryOp) -> Expression {
        Expression(unsafe { by::BinaryenUnary(self.by_module, op.to_binaryen_op(), operand.0) })
    }

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

    pub fn local_get(&self, index: u32, type_: by::BinaryenType) -> Expression {
        Expression(unsafe { by::BinaryenLocalGet(self.by_module, index, type_) })
    }

    pub fn local_set(&self, index: u32, value: Expression) -> Expression {
        Expression(unsafe { by::BinaryenLocalSet(self.by_module, index, value.0) })
    }

    pub fn unreachable(&self) -> Expression {
        Expression(unsafe { by::BinaryenUnreachable(self.by_module) })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    WrapInt64,
    ExtendSInt32,
    ExtendUInt32,

    Abs,
    Ceil,
    Floor,
    Trunc,
    Nearest,
    Sqrt,
    EqZInt32,
    ClzInt32,
    CtzInt32,
    PopcntInt32,
    EqZInt64,
    ClzInt64,
    CtzInt64,
    PopcntInt64,
}

impl UnaryOp {
    pub fn to_binaryen_op(&self) -> by::BinaryenOp {
        use UnaryOp::*;

        match self {
            WrapInt64 => unsafe { by::BinaryenWrapInt64() },
            ExtendSInt32 => unsafe { by::BinaryenExtendSInt32() },
            ExtendUInt32 => unsafe { by::BinaryenExtendUInt32() },

            Abs => unsafe { by::BinaryenAbsFloat64() },
            Ceil => unsafe { by::BinaryenCeilFloat64() },
            Floor => unsafe { by::BinaryenFloorFloat64() },
            Trunc => unsafe { by::BinaryenTruncFloat64() },
            Nearest => unsafe { by::BinaryenNearestFloat64() },
            Sqrt => unsafe { by::BinaryenSqrtFloat64() },
            EqZInt32 => unsafe { by::BinaryenEqZInt32() },
            ClzInt32 => unsafe { by::BinaryenClzInt32() },
            CtzInt32 => unsafe { by::BinaryenCtzInt32() },
            PopcntInt32 => unsafe { by::BinaryenPopcntInt32() },
            EqZInt64 => unsafe { by::BinaryenEqZInt64() },
            ClzInt64 => unsafe { by::BinaryenClzInt64() },
            CtzInt64 => unsafe { by::BinaryenCtzInt64() },
            PopcntInt64 => unsafe { by::BinaryenPopcntInt64() },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Mul,
    Sub,
    DivS,
    DivU,
    RemS,
    RemU,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    RotL,
    RotR,
    Eq,
    Ne,
    LtS,
    LtU,
    LeS,
    LeU,
    GtS,
    GtU,
    GeS,
    GeU,
}

impl BinaryOp {
    pub fn to_binaryen_op(&self) -> by::BinaryenOp {
        use BinaryOp::*;

        match self {
            Add => unsafe { by::BinaryenAddInt64() },
            Mul => unsafe { by::BinaryenMulInt64() },
            Sub => unsafe { by::BinaryenSubInt64() },
            DivS => unsafe { by::BinaryenDivSInt64() },
            DivU => unsafe { by::BinaryenDivUInt64() },
            RemS => unsafe { by::BinaryenRemSInt64() },
            RemU => unsafe { by::BinaryenRemUInt64() },
            And => unsafe { by::BinaryenAndInt64() },
            Or => unsafe { by::BinaryenOrInt64() },
            Xor => unsafe { by::BinaryenXorInt64() },
            Shl => unsafe { by::BinaryenShlInt64() },
            Shr => unsafe { by::BinaryenShrUInt64() },
            RotL => unsafe { by::BinaryenRotLInt64() },
            RotR => unsafe { by::BinaryenRotRInt64() },
            Eq => unsafe { by::BinaryenEqInt64() },
            Ne => unsafe { by::BinaryenNeInt64() },
            LtS => unsafe { by::BinaryenLtSInt64() },
            LtU => unsafe { by::BinaryenLtUInt64() },
            LeS => unsafe { by::BinaryenLeSInt64() },
            LeU => unsafe { by::BinaryenLeUInt64() },
            GtS => unsafe { by::BinaryenGtSInt64() },
            GtU => unsafe { by::BinaryenGtUInt64() },
            GeS => unsafe { by::BinaryenGeSInt64() },
            GeU => unsafe { by::BinaryenGeUInt64() },
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
