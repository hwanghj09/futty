use eframe::egui::Modifiers;
use futty_runtime::{Hierarchy, Object};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct Selection {
    ids: Vec<Uuid>,
    anchor: Option<Uuid>,
}

impl Selection {
    pub fn ids(&self) -> &[Uuid] {
        &self.ids
    }

    pub fn primary(&self) -> Option<Uuid> {
        self.ids.last().copied()
    }

    pub fn replace(&mut self, ids: impl IntoIterator<Item = Uuid>) {
        self.ids = ids.into_iter().collect();
        self.anchor = self.primary();
    }

    pub fn retain(&mut self, objects: &[Object]) {
        self.ids
            .retain(|id| objects.iter().any(|object| object.uuid == *id));
        if self.anchor.is_some_and(|id| !self.ids.contains(&id)) {
            self.anchor = self.primary();
        }
    }

    pub fn click(&mut self, target: Option<Uuid>, modifiers: Modifiers, rows: &[Uuid]) {
        let Some(id) = target else {
            if !modifiers.command && !modifiers.shift {
                self.replace(None);
            }
            return;
        };
        if modifiers.shift {
            let start = self
                .anchor
                .and_then(|anchor| rows.iter().position(|&id| id == anchor));
            let end = rows.iter().position(|&row| row == id);
            if let (Some(start), Some(end)) = (start, end) {
                if !modifiers.command {
                    self.ids.clear();
                }
                for &row in &rows[start.min(end)..=start.max(end)] {
                    if !self.ids.contains(&row) {
                        self.ids.push(row);
                    }
                }
                self.add_primary(id);
                return;
            }
            self.add_primary(id);
        } else if modifiers.command {
            if let Some(index) = self.ids.iter().position(|&row| row == id) {
                self.ids.remove(index);
            } else {
                self.ids.push(id);
            }
        } else {
            self.replace(Some(id));
        }
        self.anchor = self.primary();
    }

    fn add_primary(&mut self, id: Uuid) {
        self.ids.retain(|&row| row != id);
        self.ids.push(id);
    }

    pub fn context_target(&mut self, id: Uuid) {
        if self.ids.contains(&id) {
            self.add_primary(id);
            self.anchor = Some(id);
        } else {
            self.replace(Some(id));
        }
    }
}

pub(crate) fn selected_tree<'a>(
    objects: &'a [Object],
    hierarchy: &'a Hierarchy,
    selected: &'a [Uuid],
) -> impl Iterator<Item = (usize, bool)> + 'a {
    let mut branch = None;
    hierarchy.order.iter().filter_map(move |&(index, depth)| {
        if branch.is_some_and(|root| depth > root) {
            return Some((index, false));
        }
        branch = selected.contains(&objects[index].uuid).then_some(depth);
        branch.map(|_| (index, true))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_range_keeps_anchor_and_ctrl_toggles_membership() {
        let [a, b, c, d] = std::array::from_fn(|_| Uuid::new_v4());
        let rows = [a, b, c, d];
        let mut selection = Selection::default();
        selection.click(Some(b), Modifiers::NONE, &rows);
        selection.click(Some(d), Modifiers::SHIFT, &rows);
        assert_eq!(selection.ids(), &[b, c, d]);
        selection.click(Some(a), Modifiers::SHIFT, &rows);
        assert_eq!(selection.ids(), &[b, a]);
        selection.click(Some(b), Modifiers::COMMAND, &rows);
        assert_eq!(selection.ids(), &[a]);
        selection.click(None, Modifiers::COMMAND, &rows);
        assert_eq!(selection.primary(), Some(a));
        selection.click(None, Modifiers::NONE, &rows);
        assert!(selection.ids().is_empty());
    }

    #[test]
    fn right_click_keeps_a_group_only_when_the_target_is_already_selected() {
        let [a, b, c] = std::array::from_fn(|_| Uuid::new_v4());
        let mut selection = Selection::default();
        selection.replace([a, b]);
        selection.context_target(a);
        assert_eq!(selection.ids(), &[b, a]);
        selection.context_target(c);
        assert_eq!(selection.ids(), &[c]);
    }
}
