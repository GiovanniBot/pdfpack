//! Seleção múltipla e reordenação da lista: as contas, sem a janela, para poderem ser testadas.

use std::collections::HashSet;

/// A seleção e a âncora depois de um clique na linha `id`. `order` é a lista na ordem da tela.
///
/// Sem teclas, fica só a linha clicada. Com Ctrl, ela entra ou sai da seleção. Com Shift, tudo
/// entre a âncora (o último clique simples ou com Ctrl) e ela; Ctrl+Shift soma em vez de trocar.
pub fn click(order: &[u64], picked: &HashSet<u64>, anchor: Option<u64>, id: u64, ctrl: bool, shift: bool) -> (HashSet<u64>, Option<u64>) {
    let at = |x: u64| order.iter().position(|o| *o == x);
    if shift && let (Some(a), Some(b)) = (anchor.and_then(at), at(id)) {
        let mut out = if ctrl { picked.clone() } else { HashSet::new() };
        out.extend(order.get(a.min(b)..=a.max(b)).unwrap_or_default().iter().copied());
        return (out, anchor);
    }
    if ctrl {
        let mut out = picked.clone();
        if !out.remove(&id) {
            out.insert(id);
        }
        return (out, Some(id));
    }
    (HashSet::from([id]), Some(id))
}

/// A ordem depois de levar `moving`, juntos e na ordem em que estavam, para antes da posição `to`.
pub fn moved(order: &[u64], moving: &HashSet<u64>, to: usize) -> Vec<u64> {
    let before = order.iter().take(to).filter(|id| !moving.contains(id)).count();
    let mut out: Vec<u64> = order.iter().copied().filter(|id| !moving.contains(id)).collect();
    let block: Vec<u64> = order.iter().copied().filter(|id| moving.contains(id)).collect();
    let at = before.min(out.len());
    out.splice(at..at, block);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[u64]) -> HashSet<u64> {
        ids.iter().copied().collect()
    }

    #[test]
    fn a_plain_click_selects_only_that_row() {
        let (picked, anchor) = click(&[1, 2, 3], &set(&[1, 3]), Some(1), 2, false, false);
        assert_eq!(picked, set(&[2]));
        assert_eq!(anchor, Some(2));
    }

    #[test]
    fn ctrl_adds_and_removes_one_row() {
        let (picked, anchor) = click(&[1, 2, 3], &set(&[1]), Some(1), 3, true, false);
        assert_eq!(picked, set(&[1, 3]));
        assert_eq!(anchor, Some(3));
        let (picked, _) = click(&[1, 2, 3], &picked, anchor, 1, true, false);
        assert_eq!(picked, set(&[3]));
    }

    #[test]
    fn shift_selects_the_range_from_the_anchor_in_either_direction() {
        let order = [1, 2, 3, 4, 5];
        let (picked, anchor) = click(&order, &set(&[2]), Some(2), 4, false, true);
        assert_eq!(picked, set(&[2, 3, 4]));
        // A âncora não anda: outro Shift+clique troca o intervalo, não o estende a partir do fim.
        assert_eq!(anchor, Some(2));
        let (picked, _) = click(&order, &picked, anchor, 1, false, true);
        assert_eq!(picked, set(&[1, 2]));
        let (picked, _) = click(&order, &set(&[5]), Some(2), 3, true, true);
        assert_eq!(picked, set(&[2, 3, 5]));
    }

    #[test]
    fn shift_without_an_anchor_is_a_plain_click() {
        let (picked, anchor) = click(&[1, 2, 3], &HashSet::new(), None, 3, false, true);
        assert_eq!(picked, set(&[3]));
        assert_eq!(anchor, Some(3));
        // Uma âncora que saiu da lista também não serve.
        assert_eq!(click(&[1, 2, 3], &HashSet::new(), Some(9), 2, false, true).0, set(&[2]));
    }

    #[test]
    fn rows_move_together_and_keep_their_order() {
        let order = [1, 2, 3, 4, 5];
        assert_eq!(moved(&order, &set(&[2]), 4), [1, 3, 4, 2, 5]);
        assert_eq!(moved(&order, &set(&[4]), 0), [4, 1, 2, 3, 5]);
        assert_eq!(moved(&order, &set(&[1, 4]), 3), [2, 3, 1, 4, 5]);
        assert_eq!(moved(&order, &set(&[2, 5]), 5), [1, 3, 4, 2, 5]);
        // Soltar onde já estava, ou além do fim, não quebra nem muda nada à toa.
        assert_eq!(moved(&order, &set(&[3]), 2), order);
        assert_eq!(moved(&order, &set(&[3]), 3), order);
        assert_eq!(moved(&order, &set(&[5]), 99), order);
        assert_eq!(moved(&order, &set(&[1, 2, 3, 4, 5]), 2), order);
    }
}
