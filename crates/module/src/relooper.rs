use std::rc::Rc;

use binaryen::ffi as by;

use crate::{
    core::{Module, ModuleInner},
    expression::Expression,
};

impl Module {
    pub fn relooper(&mut self) -> Relooper {
        Relooper {
            module: self.inner_rc(),
            by_relooper: unsafe { by::RelooperCreate(self.module_ptr()) },
        }
    }
}

#[derive(Debug)]
pub struct Relooper {
    module: Rc<ModuleInner>,
    by_relooper: by::RelooperRef,
}

impl Relooper {
    pub fn add_block(&mut self, code: Expression) -> RelooperBlock {
        RelooperBlock {
            by_block: unsafe { by::RelooperAddBlock(self.by_relooper, code.ptr()) },
        }
    }

    pub fn branch(&self, from: &RelooperBlock, to: &RelooperBlock, condition: Option<Expression>) {
        let condition_ptr = match condition {
            Some(cond) => cond.ptr(),
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
        Expression {
            _module: self.module,
            ptr: unsafe { by::RelooperRenderAndDispose(self.by_relooper, entry.by_block, 0) },
        }
    }
}

#[derive(Debug)]
pub struct RelooperBlock {
    by_block: by::RelooperBlockRef,
}
