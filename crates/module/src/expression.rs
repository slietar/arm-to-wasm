use std::{
    ffi::CString,
    rc::Rc,
};

use binaryen::ffi as by;

use crate::{Type, core::{Module, ModuleInner}};

#[derive(Debug, Clone)]
pub struct Expression {
    pub(crate) _module: Rc<ModuleInner>,
    pub(crate) ptr: by::BinaryenExpressionRef,
}

impl Expression {
    pub(crate) fn ptr(&self) -> by::BinaryenExpressionRef {
        self.ptr
    }
}

impl Module {
    pub fn binary(&self, operand1: Expression, operand2: Expression, op: BinaryOp) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe {
                by::BinaryenBinary(
                    self.module_ptr(),
                    op.to_binaryen_op(),
                    operand1.ptr(),
                    operand2.ptr(),
                )
            },
        }
    }

    pub fn call(
        &self,
        target: &str,
        arguments: &[Expression],
        return_type: Type,
    ) -> Expression {
        let target_cstr = CString::new(target).unwrap();

        let mut argument_ptrs: Vec<_> = arguments.iter().map(Expression::ptr).collect();

        Expression {
            _module: self.inner_rc(),
            ptr: unsafe {
                by::BinaryenCall(
                    self.module_ptr(),
                    target_cstr.as_ptr(),
                    argument_ptrs.as_mut_ptr(),
                    argument_ptrs.len() as u32,
                    return_type.ptr,
                )
            },
        }
    }

    pub fn const_<T: ToBinaryenLiteral>(&self, value: T) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenConst(self.module_ptr(), value.to_literal()) },
        }
    }

    pub fn drop(&self, expr: Expression) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenDrop(self.module_ptr(), expr.ptr()) },
        }
    }

    pub fn unary(&self, operand: Expression, op: UnaryOp) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenUnary(self.module_ptr(), op.to_binaryen_op(), operand.ptr()) },
        }
    }

    pub fn block(&self, type_: Type, expressions: &[Expression]) -> Expression {
        let mut expression_ptrs: Vec<_> = expressions.iter().map(Expression::ptr).collect();

        Expression {
            _module: self.inner_rc(),
            ptr: unsafe {
                by::BinaryenBlock(
                    self.module_ptr(),
                    std::ptr::null(),
                    expression_ptrs.as_mut_ptr(),
                    expression_ptrs.len() as u32,
                    type_.ptr,
                )
            },
        }
    }

    pub fn function(
        &self,
        name: &str,
        params: &[Type],
        result: Type,
        locals: &[Type],
        body: Expression,
    ) -> by::BinaryenFunctionRef {
        let name_cstr = CString::new(name).unwrap();
        let mut local_ptrs: Vec<_> = locals.iter().map(|t| t.ptr).collect();

        unsafe {
            by::BinaryenAddFunction(
                self.module_ptr(),
                name_cstr.as_ptr(),
                self.tuple_type(params).ptr,
                result.ptr,
                local_ptrs.as_mut_ptr(),
                local_ptrs.len() as u32,
                body.ptr(),
            )
        }
    }

    pub fn local_get(&self, index: u32, type_: Type) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenLocalGet(self.module_ptr(), index, type_.ptr) },
        }
    }

    pub fn local_set(&self, index: u32, value: Expression) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenLocalSet(self.module_ptr(), index, value.ptr()) },
        }
    }

    pub fn return_(&self, value: Expression) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenReturn(self.module_ptr(), value.ptr()) },
        }
    }

    pub fn tuple(&self, operands: &[Expression]) -> Expression {
        let mut operand_ptrs: Vec<_> = operands.iter().map(Expression::ptr).collect();

        Expression {
            _module: self.inner_rc(),
            ptr: unsafe {
                by::BinaryenTupleMake(
                    self.module_ptr(),
                    operand_ptrs.as_mut_ptr(),
                    operand_ptrs.len() as u32,
                )
            },
        }
    }

    pub fn tuple_extract(&self, tuple: Expression, index: u32) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenTupleExtract(self.module_ptr(), tuple.ptr(), index) },
        }
    }

    pub fn unreachable(&self) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenUnreachable(self.module_ptr()) },
        }
    }

    pub fn nop(&self) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe { by::BinaryenNop(self.module_ptr()) },
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
    AddInt64,
    MulInt64,
    SubInt64,
    DivSInt64,
    DivUInt64,
    RemSInt64,
    RemUInt64,
    AndInt64,
    OrInt64,
    XorInt64,
    ShlInt64,
    ShrSInt64,
    ShrUInt64,
    RotLInt64,
    RotRInt64,
    EqInt64,
    NeInt64,
    LtSInt64,
    LtUInt64,
    LeSInt64,
    LeUInt64,
    GtSInt64,
    GtUInt64,
    GeSInt64,
    GeUInt64,
    AddInt32,
    MulInt32,
    SubInt32,
    DivSInt32,
    DivUInt32,
    RemSInt32,
    RemUInt32,
    AndInt32,
    OrInt32,
    XorInt32,
    ShlInt32,
    ShrSInt32,
    ShrUInt32,
    RotLInt32,
    RotRInt32,
    EqInt32,
    NeInt32,
    LtSInt32,
    LtUInt32,
    LeSInt32,
    LeUInt32,
    GtSInt32,
    GtUInt32,
    GeSInt32,
    GeUInt32,
}

impl BinaryOp {
    pub fn to_binaryen_op(&self) -> by::BinaryenOp {
        use BinaryOp::*;

        match self {
            AddInt64 => unsafe { by::BinaryenAddInt64() },
            MulInt64 => unsafe { by::BinaryenMulInt64() },
            SubInt64 => unsafe { by::BinaryenSubInt64() },
            DivSInt64 => unsafe { by::BinaryenDivSInt64() },
            DivUInt64 => unsafe { by::BinaryenDivUInt64() },
            RemSInt64 => unsafe { by::BinaryenRemSInt64() },
            RemUInt64 => unsafe { by::BinaryenRemUInt64() },
            AndInt64 => unsafe { by::BinaryenAndInt64() },
            OrInt64 => unsafe { by::BinaryenOrInt64() },
            XorInt64 => unsafe { by::BinaryenXorInt64() },
            ShlInt64 => unsafe { by::BinaryenShlInt64() },
            ShrSInt64 => unsafe { by::BinaryenShrSInt64() },
            ShrUInt64 => unsafe { by::BinaryenShrUInt64() },
            RotLInt64 => unsafe { by::BinaryenRotLInt64() },
            RotRInt64 => unsafe { by::BinaryenRotRInt64() },
            EqInt64 => unsafe { by::BinaryenEqInt64() },
            NeInt64 => unsafe { by::BinaryenNeInt64() },
            LtSInt64 => unsafe { by::BinaryenLtSInt64() },
            LtUInt64 => unsafe { by::BinaryenLtUInt64() },
            LeSInt64 => unsafe { by::BinaryenLeSInt64() },
            LeUInt64 => unsafe { by::BinaryenLeUInt64() },
            GtSInt64 => unsafe { by::BinaryenGtSInt64() },
            GtUInt64 => unsafe { by::BinaryenGtUInt64() },
            GeSInt64 => unsafe { by::BinaryenGeSInt64() },
            GeUInt64 => unsafe { by::BinaryenGeUInt64() },
            AddInt32 => unsafe { by::BinaryenAddInt32() },
            MulInt32 => unsafe { by::BinaryenMulInt32() },
            SubInt32 => unsafe { by::BinaryenSubInt32() },
            DivSInt32 => unsafe { by::BinaryenDivSInt32() },
            DivUInt32 => unsafe { by::BinaryenDivUInt32() },
            RemSInt32 => unsafe { by::BinaryenRemSInt32() },
            RemUInt32 => unsafe { by::BinaryenRemUInt32() },
            AndInt32 => unsafe { by::BinaryenAndInt32() },
            OrInt32 => unsafe { by::BinaryenOrInt32() },
            XorInt32 => unsafe { by::BinaryenXorInt32() },
            ShlInt32 => unsafe { by::BinaryenShlInt32() },
            ShrSInt32 => unsafe { by::BinaryenShrSInt32() },
            ShrUInt32 => unsafe { by::BinaryenShrUInt32() },
            RotLInt32 => unsafe { by::BinaryenRotLInt32() },
            RotRInt32 => unsafe { by::BinaryenRotRInt32() },
            EqInt32 => unsafe { by::BinaryenEqInt32() },
            NeInt32 => unsafe { by::BinaryenNeInt32() },
            LtSInt32 => unsafe { by::BinaryenLtSInt32() },
            LtUInt32 => unsafe { by::BinaryenLtUInt32() },
            LeSInt32 => unsafe { by::BinaryenLeSInt32() },
            LeUInt32 => unsafe { by::BinaryenLeUInt32() },
            GtSInt32 => unsafe { by::BinaryenGtSInt32() },
            GtUInt32 => unsafe { by::BinaryenGtUInt32() },
            GeSInt32 => unsafe { by::BinaryenGeSInt32() },
            GeUInt32 => unsafe { by::BinaryenGeUInt32() },
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
