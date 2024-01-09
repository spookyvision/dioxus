use std::ops::Deref;

use crate::{
    any_props::AnyProps,
    innerlude::{DirtyScope, ElementRef, VComponent, VNodeMount, WriteMutations},
    nodes::RenderReturn,
    nodes::VNode,
    scopes::ScopeId,
    virtual_dom::VirtualDom,
};

impl VirtualDom {
    pub(crate) fn diff_scope(
        &mut self,
        to: &mut impl WriteMutations,
        scope: ScopeId,
        new_nodes: RenderReturn,
    ) {
        self.runtime.scope_stack.borrow_mut().push(scope);
        let scope_state = &mut self.scopes[scope.0];
        // Load the old and new bump arenas
        let new = &new_nodes;
        let old = scope_state.last_rendered_node.take().unwrap();

        use RenderReturn::{Aborted, Ready};

        let (Ready(old) | Aborted(old), Ready(new) | Aborted(new)) = (&old, new);
        old.diff_node(new, self, to);

        let scope_state = &mut self.scopes[scope.0];
        scope_state.last_rendered_node = Some(new_nodes);

        self.runtime.scope_stack.borrow_mut().pop();
    }

    /// Create a new template [`VNode`] and write it to the [`Mutations`] buffer.
    ///
    /// This method pushes the ScopeID to the internal scopestack and returns the number of nodes created.
    pub(crate) fn create_scope(
        &mut self,
        to: &mut impl WriteMutations,
        scope: ScopeId,
        new_node: &VNode,
        parent: Option<ElementRef>,
    ) -> usize {
        self.runtime.scope_stack.borrow_mut().push(scope);
        let nodes = new_node.create(self, to, parent);
        self.runtime.scope_stack.borrow_mut().pop();
        nodes
    }
}

impl VNode {
    pub(crate) fn diff_vcomponent(
        &self,
        mount: &mut VNodeMount,
        idx: usize,
        new: &VComponent,
        old: &VComponent,
        scope_id: ScopeId,
        parent: Option<ElementRef>,
        dom: &mut VirtualDom,
        to: &mut impl WriteMutations,
    ) {
        // Replace components that have different render fns
        if old.render_fn != new.render_fn {
            return self.replace_vcomponent(mount, idx, new, parent, dom, to);
        }

        // copy out the box for both
        let old_scope = &mut dom.scopes[scope_id.0];
        let old_props = old_scope.props.deref();
        let new_props: &dyn AnyProps = new.props.deref();

        // If the props are static, then we try to memoize by setting the new with the old
        // The target scopestate still has the reference to the old props, so there's no need to update anything
        // This also implicitly drops the new props since they're not used
        if old_props.memoize(new_props.props()) {
            tracing::trace!(
                "Memoized props for component {:#?} ({})",
                scope_id,
                old_scope.context().name
            );
            return;
        }

        // First, move over the props from the old to the new, dropping old props in the process
        dom.scopes[scope_id.0].props = new.props.clone();

        // Now run the component and diff it
        let new = dom.run_scope(scope_id);
        dom.diff_scope(to, scope_id, new);

        let height = dom.runtime.get_context(scope_id).unwrap().height;
        dom.dirty_scopes.remove(&DirtyScope {
            height,
            id: scope_id,
        });
    }
    fn replace_vcomponent(
        &self,
        mount: &mut VNodeMount,
        idx: usize,
        new: &VComponent,
        parent: Option<ElementRef>,
        dom: &mut VirtualDom,
        to: &mut impl WriteMutations,
    ) {
        let scope = ScopeId(mount.mounted_dynamic_nodes[idx]);

        let _m = self.create_component_node(mount, idx, new, parent, dom, to);

        // TODO: Instead of *just* removing it, we can use the replace mutation
        dom.remove_component_node(to, scope, true);

        todo!()
    }

    pub(super) fn create_component_node(
        &self,
        _mount: &mut VNodeMount,
        _idx: usize,
        component: &VComponent,
        parent: Option<ElementRef>,
        dom: &mut VirtualDom,
        to: &mut impl WriteMutations,
    ) -> usize {
        // Load up a ScopeId for this vcomponent. If it's already mounted, then we can just use that
        let scope = dom
            .new_scope(component.props.clone(), component.name)
            .context()
            .id;

        let new = dom.run_scope(scope);

        dom.scopes[scope.0].last_rendered_node = Some(new.clone());

        dom.create_scope(to, scope, &new, parent)
    }
}