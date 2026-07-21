use toasty_core::stmt;

use crate::engine::{
    eval, exec,
    mir::{self, LogicalPlan},
};

/// Selects one updated model by its pre-update primary key.
#[derive(Debug)]
pub(crate) struct ReturnFirst {
    pub(crate) input: mir::NodeId,
    pub(crate) selector: Option<mir::NodeId>,
    pub(crate) key: Option<eval::Func>,
    pub(crate) required: bool,
    pub(crate) ty: stmt::Type,
}

impl ReturnFirst {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::ReturnFirst {
        let input = logical_plan[self.input].var.get().unwrap();
        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        exec::ReturnFirst {
            input,
            selector: self
                .selector
                .map(|selector| logical_plan[selector].var.get().unwrap()),
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            key: self.key.clone(),
            required: self.required,
        }
    }
}

impl From<ReturnFirst> for mir::Node {
    fn from(value: ReturnFirst) -> Self {
        mir::Operation::ReturnFirst(value).into()
    }
}
