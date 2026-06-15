use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
};

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
        Expression(unsafe {
            by::BinaryenBinary(self.by_module, op.to_binaryen_op(), operand1.0, operand2.0)
        })
    }

    pub fn call(
        &self,
        target: &str,
        arguments: &[Expression],
        return_type: by::BinaryenType,
    ) -> Expression {
        let target_cstr = CString::new(target).unwrap();

        Expression(unsafe {
            by::BinaryenCall(
                self.by_module,
                target_cstr.as_ptr(),
                arguments.as_ptr() as *mut by::BinaryenExpressionRef,
                arguments.len() as u32,
                return_type,
            )
        })
    }

    pub fn drop(&self, expr: Expression) -> Expression {
        Expression(unsafe { by::BinaryenDrop(self.by_module, expr.0) })
    }

    pub fn unary(&self, operand: Expression, op: UnaryOp) -> Expression {
        Expression(unsafe { by::BinaryenUnary(self.by_module, op.to_binaryen_op(), operand.0) })
    }

    pub fn block(&self, type_: by::BinaryenType, expressions: &[Expression]) -> Expression {
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
                self.tuple_type(params),
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

    pub fn return_(&self, value: Expression) -> Expression {
        Expression(unsafe { by::BinaryenReturn(self.by_module, value.0) })
    }

    pub fn tuple(&self, operands: &[Expression]) -> Expression {
        Expression(unsafe {
            by::BinaryenTupleMake(
                self.by_module,
                operands.as_ptr() as *mut by::BinaryenExpressionRef,
                operands.len() as u32,
            )
        })
    }

    pub fn tuple_extract(&self, tuple: Expression, index: u32) -> Expression {
        Expression(unsafe { by::BinaryenTupleExtract(self.by_module, tuple.0, index) })
    }

    pub fn unreachable(&self) -> Expression {
        Expression(unsafe { by::BinaryenUnreachable(self.by_module) })
    }

    pub fn tuple_type(&self, types: &[by::BinaryenType]) -> by::BinaryenType {
        unsafe {
            by::BinaryenTypeCreate(types.as_ptr() as *mut by::BinaryenType, types.len() as u32)
        }
    }

    pub fn nop(&self) -> Expression {
        Expression(unsafe { by::BinaryenNop(self.by_module) })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadVariant {
    I32,
    I64,
    I32L8 { signed: bool },
    I32L16 { signed: bool },
    I64L8 { signed: bool },
    I64L16 { signed: bool },
    I64L32 { signed: bool },
}

impl LoadVariant {
    pub fn byte_count(&self) -> u32 {
        match self {
            LoadVariant::I32 => 4,
            LoadVariant::I64 => 8,
            LoadVariant::I32L8 { .. } => 1,
            LoadVariant::I32L16 { .. } => 2,
            LoadVariant::I64L8 { .. } => 1,
            LoadVariant::I64L16 { .. } => 2,
            LoadVariant::I64L32 { .. } => 4,
        }
    }

    pub fn alignment(&self) -> u32 {
        match self {
            LoadVariant::I32 => 4,
            LoadVariant::I64 => 8,
            LoadVariant::I32L8 { .. } => 1,
            LoadVariant::I32L16 { .. } => 2,
            LoadVariant::I64L8 { .. } => 1,
            LoadVariant::I64L16 { .. } => 2,
            LoadVariant::I64L32 { .. } => 4,
        }
    }

    pub fn signed(&self) -> bool {
        match self {
            LoadVariant::I32 => true,
            LoadVariant::I64 => true,
            LoadVariant::I32L8 { signed } => *signed,
            LoadVariant::I32L16 { signed } => *signed,
            LoadVariant::I64L8 { signed } => *signed,
            LoadVariant::I64L16 { signed } => *signed,
            LoadVariant::I64L32 { signed } => *signed,
        }
    }

    pub fn type_(&self, module: &Module) -> by::BinaryenType {
        match self {
            LoadVariant::I32 => module.i32(),
            LoadVariant::I64 => module.i64(),
            LoadVariant::I32L8 { .. } => module.i32(),
            LoadVariant::I32L16 { .. } => module.i32(),
            LoadVariant::I64L8 { .. } => module.i64(),
            LoadVariant::I64L16 { .. } => module.i64(),
            LoadVariant::I64L32 { .. } => module.i64(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreVariant {
    I32,
    I64,
    I32L8,
    I32L16,
    I64L8,
    I64L16,
    I64L32,
}

impl StoreVariant {
    pub fn byte_count(&self) -> u32 {
        match self {
            StoreVariant::I32 => 4,
            StoreVariant::I64 => 8,
            StoreVariant::I32L8 => 1,
            StoreVariant::I32L16 => 2,
            StoreVariant::I64L8 => 1,
            StoreVariant::I64L16 => 2,
            StoreVariant::I64L32 => 4,
        }
    }

    pub fn alignment(&self) -> u32 {
        match self {
            StoreVariant::I32 => 4,
            StoreVariant::I64 => 8,
            StoreVariant::I32L8 => 1,
            StoreVariant::I32L16 => 2,
            StoreVariant::I64L8 => 1,
            StoreVariant::I64L16 => 2,
            StoreVariant::I64L32 => 4,
        }
    }

    pub fn type_(&self, module: &Module) -> by::BinaryenType {
        match self {
            StoreVariant::I32 => module.i32(),
            StoreVariant::I64 => module.i64(),
            StoreVariant::I32L8 => module.i32(),
            StoreVariant::I32L16 => module.i32(),
            StoreVariant::I64L8 => module.i64(),
            StoreVariant::I64L16 => module.i64(),
            StoreVariant::I64L32 => module.i64(),
        }
    }
}

impl Module {
    fn load_internal(
        &self,
        address: Expression,
        offset: u32,
        alignment: u32,
        byte_count: u32,
        type_: by::BinaryenType,
        memory_name: &CStr,
        signed: bool,
    ) -> Expression {
        Expression(unsafe {
            by::BinaryenLoad(
                self.by_module,
                byte_count,
                signed,
                offset,
                alignment,
                type_,
                address.0,
                memory_name.as_ptr(),
            )
        })
    }

    pub fn load(
        &self,
        variant: LoadVariant,
        address: Expression,
        offset: u32,
        alignment: u32,
        memory_name: &CStr,
    ) -> Expression {
        self.load_internal(
            address,
            offset,
            alignment,
            variant.byte_count(),
            variant.type_(self),
            memory_name,
            variant.signed(),
        )
    }

    fn store_internal(
        &self,
        value: Expression,
        address: Expression,
        offset: u32,
        alignment: u32,
        byte_count: u32,
        type_: by::BinaryenType,
        memory_name: &CStr,
    ) -> Expression {
        Expression(unsafe {
            by::BinaryenStore(
                self.by_module,
                byte_count,
                offset,
                alignment,
                address.0,
                value.0,
                type_,
                memory_name.as_ptr(),
            )
        })
    }

    pub fn store(
        &self,
        variant: StoreVariant,
        value: Expression,
        address: Expression,
        offset: u32,
        alignment: u32,
        memory_name: &CStr,
    ) -> Expression {
        self.store_internal(
            value,
            address,
            offset,
            alignment,
            variant.byte_count(),
            variant.type_(self),
            memory_name,
        )
    }
}

impl Module {
    pub fn import_function(
        &self,
        internal_name: &str,
        external_module_name: &str,
        external_function_name: &str,
        params: &[by::BinaryenType],
        result: by::BinaryenType,
    ) {
        let internal_name_cstr = CString::new(internal_name).unwrap();
        let external_module_name_cstr = CString::new(external_module_name).unwrap();
        let external_function_name_cstr = CString::new(external_function_name).unwrap();

        unsafe {
            by::BinaryenAddFunctionImport(
                self.by_module,
                internal_name_cstr.as_ptr(),
                external_module_name_cstr.as_ptr(),
                external_function_name_cstr.as_ptr(),
                self.tuple_type(params),
                result,
            )
        }
    }

    pub fn export_function(&self, internal_name: &str, external_name: &str) {
        let internal_name_cstr = CString::new(internal_name).unwrap();
        let external_name_cstr = CString::new(external_name).unwrap();

        unsafe {
            by::BinaryenAddFunctionExport(
                self.by_module,
                internal_name_cstr.as_ptr(),
                external_name_cstr.as_ptr(),
            );
        }
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
    ShrInt64,
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
    ShrInt32,
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
            ShrInt64 => unsafe { by::BinaryenShrUInt64() },
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
            ShrInt32 => unsafe { by::BinaryenShrUInt32() },
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

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Expression(by::BinaryenExpressionRef);

impl Expression {
    pub unsafe fn extract(&self) -> by::BinaryenExpressionRef {
        self.0
    }
}

#[derive(Debug)]
pub struct Relooper {
    by_relooper: by::RelooperRef,
}

impl Relooper {
    pub fn add_block(&mut self, code: Expression) -> RelooperBlock {
        RelooperBlock {
            by_block: unsafe { by::RelooperAddBlock(self.by_relooper, code.0) },
        }
    }

    pub fn branch(&self, from: &RelooperBlock, to: &RelooperBlock, condition: Option<Expression>) {
        let condition_ptr = match condition {
            Some(cond) => cond.0,
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

    pub fn finish(self, entry: &RelooperBlock) -> Expression {
        Expression(unsafe { by::RelooperRenderAndDispose(self.by_relooper, entry.by_block, 0) })
    }
}

#[derive(Debug)]
pub struct RelooperBlock {
    by_block: by::RelooperBlockRef,
}
