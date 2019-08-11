use super::*;
use bit_set::BitSet;

/// Create a **BitSet** from a list of elements.
///
/// ## Example
///
/// ```
/// #[macro_use] extern crate fba_quorum_analyzer;
///
/// let set = bitset!{23, 42};
/// assert!(set.contains(23));
/// assert!(set.contains(42));
/// assert!(!set.contains(100));
/// ```
#[macro_export]
macro_rules! bitset {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(bitset!(@single $rest)),*]));

    ($($key:expr,)+) => { bitset!($($key),+) };
    ($($key:expr),*) => {
        {
            let _cap = bitset!(@count $($key),*);
            let mut _set = ::bit_set::BitSet::with_capacity(_cap);
            $(
                let _ = _set.insert($key);
            )*
            _set
        }
    };
}

impl Network {
    fn is_quorum(&self, node_set: &BitSet) -> bool {
        !node_set.is_empty()
            && node_set
                .into_iter()
                .find(|x| !self.nodes[*x].is_quorum(&node_set))
                == None
    }
}
impl Node {
    pub fn is_quorum(&self, node_set: &BitSet) -> bool {
        self.quorum_set.is_quorum(node_set)
    }
}
impl QuorumSet {
    pub fn is_quorum(&self, node_set: &BitSet) -> bool {
        let found_validator_matches = self
            .validators
            .iter()
            .filter(|x| node_set.contains(**x))
            .take(self.threshold)
            .count();
        let found_inner_quorum_set_matches = self
            .inner_quorum_sets
            .iter()
            .filter(|x| x.is_quorum(node_set))
            .take(self.threshold - found_validator_matches)
            .count();

        found_validator_matches + found_inner_quorum_set_matches == self.threshold
    }
}

pub fn has_quorum_intersection(network: &Network) -> bool {
    all_node_sets_interesect(&get_minimal_quorums(network))
}

pub fn get_minimal_quorums(network: &Network) -> Vec<BitSet> {
    fn get_minimal_quorums_step(
        unprocessed: &mut Vec<NodeID>,
        selection: &mut BitSet,
        network: &Network,
    ) -> Vec<BitSet> {
        let mut result: Vec<BitSet> = vec![];

        if network.is_quorum(selection) {
            result.push(selection.clone());
        } else if let Some(current_candidate) = unprocessed.pop() {
            selection.insert(current_candidate);
            result.extend(get_minimal_quorums_step(unprocessed, selection, network));

            selection.remove(current_candidate);
            result.extend(get_minimal_quorums_step(unprocessed, selection, network));

            unprocessed.push(current_candidate);
        }
        // TODO pruning / knowing when to stop
        result
    }
    let n = network.nodes.len();
    let mut unprocessed: Vec<NodeID> = (0..n).collect();
    unprocessed.reverse(); // will be used as LIFO queue

    let mut selection = BitSet::with_capacity(n);

    let quorums = get_minimal_quorums_step(&mut unprocessed, &mut selection, network);
    remove_non_minimal_node_sets(quorums)
}

pub fn all_node_sets_interesect(node_sets: &[BitSet]) -> bool {
    node_sets
        .iter()
        .enumerate()
        .all(|(i, x)| node_sets.iter().skip(i + 1).all(|y| !x.is_disjoint(y)))
}

fn remove_non_minimal_node_sets(node_sets: Vec<BitSet>) -> Vec<BitSet> {
    let mut node_sets = node_sets;
    let mut minimal_node_sets: Vec<BitSet> = vec![];

    node_sets.sort_by(|x, y| x.len().cmp(&y.len()));

    for node_set in node_sets.into_iter() {
        if minimal_node_sets
            .iter()
            .find(|x| x.is_subset(&node_set))
            .is_none()
        {
            minimal_node_sets.push(node_set);
        }
    }
    minimal_node_sets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node(validators: &[NodeID], threshold: usize) -> Node {
        Node {
            public_key: Default::default(),
            quorum_set: QuorumSet {
                threshold,
                validators: validators.iter().copied().collect(),
                inner_quorum_sets: vec![],
            },
        }
    }

    #[test]
    fn is_quorum_if_not_quorum() {
        let node = test_node(&[0, 1, 2], 3);
        let node_set = &[1, 2, 3].iter().copied().collect();
        assert!(!node.is_quorum(&node_set));
    }

    #[test]
    fn is_quorum_if_quorum() {
        let node = test_node(&[0, 1, 2], 2);
        let node_set = &[1, 2, 3].iter().copied().collect();
        assert!(node.is_quorum(&node_set));
    }

    #[test]
    fn is_quorum_with_inner_quorum_sets() {
        let mut node = test_node(&[0, 1], 3);
        node.quorum_set.inner_quorum_sets = vec![
            QuorumSet {
                threshold: 2,
                validators: vec![2, 3, 4],
                inner_quorum_sets: vec![],
            },
            QuorumSet {
                threshold: 2,
                validators: vec![4, 5, 6],
                inner_quorum_sets: vec![],
            },
        ];
        let not_quorum = &[1, 2, 3].iter().copied().collect();
        let quorum = &[0, 3, 4, 5].iter().copied().collect();
        assert!(!node.is_quorum(&not_quorum));
        assert!(node.is_quorum(&quorum));
    }

    #[test]
    fn is_quorum_for_network() {
        let network = Network::from_json_file("test_data/correct_trivial.json");

        assert!(network.is_quorum(&vec![0, 1].into_iter().collect()));
        assert!(!network.is_quorum(&vec![0].into_iter().collect()));
    }

    #[test]
    fn empty_set_is_not_quorum() {
        let node = test_node(&[0, 1, 2], 2);
        assert!(!node.is_quorum(&BitSet::new()));

        let network = Network::from_json_file("test_data/correct_trivial.json");
        assert!(!network.is_quorum(&BitSet::new()));
    }

    #[test]
    fn get_minimal_quorums_correct_trivial() {
        let network = Network::from_json_file("test_data/correct_trivial.json");

        let expected = vec![bitset! {0, 1}, bitset! {0, 2}, bitset! {1, 2}];
        let actual = get_minimal_quorums(&network);

        assert_eq!(expected, actual);
    }

    #[test]
    fn get_minimal_quorums_broken_trivial() {
        let network = Network::from_json_file("test_data/broken_trivial.json");

        let expected = vec![bitset! {0}, bitset! {1, 2}];
        let actual = get_minimal_quorums(&network);

        assert_eq!(expected, actual);
    }

    #[test]
    fn get_minimal_quorums_broken_trivial_reversed_node_ids() {
        let mut network = Network::from_json_file("test_data/broken_trivial.json");
        network.nodes.reverse();

        let expected = vec![bitset! {2}, bitset! {0, 1}];
        let actual = get_minimal_quorums(&network);

        assert_eq!(expected, actual);
    }

    #[test]
    fn node_set_interesections() {
        assert!(all_node_sets_interesect(&vec![
            bitset! {0,1},
            bitset! {0,2},
            bitset! {1,2}
        ]));
        assert!(!all_node_sets_interesect(&vec![bitset! {0}, bitset! {1,2}]));
    }

    #[test]
    fn has_quorum_intersection_trivial() {
        let correct = Network::from_json_file("test_data/correct_trivial.json");
        let broken = Network::from_json_file("test_data/broken_trivial.json");

        assert!(has_quorum_intersection(&correct));
        assert!(!has_quorum_intersection(&broken));
    }
}
