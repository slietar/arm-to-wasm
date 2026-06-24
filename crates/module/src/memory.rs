use std::ffi::CStr;

use crate::{Expression, Module, Type};
use binaryen::ffi as by;

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

    pub fn type_(&self, module: &Module) -> Type {
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

    pub fn type_(&self, module: &Module) -> Type {
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
        type_: Type,
        memory_name: &CStr,
        signed: bool,
    ) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe {
                by::BinaryenLoad(
                    self.module_ptr(),
                    byte_count,
                    signed,
                    offset,
                    alignment,
                    type_.ptr,
                    address.ptr(),
                    memory_name.as_ptr(),
                )
            },
        }
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
        type_: Type,
        memory_name: &CStr,
    ) -> Expression {
        Expression {
            _module: self.inner_rc(),
            ptr: unsafe {
                by::BinaryenStore(
                    self.module_ptr(),
                    byte_count,
                    offset,
                    alignment,
                    address.ptr(),
                    value.ptr(),
                    type_.ptr,
                    memory_name.as_ptr(),
                )
            },
        }
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
