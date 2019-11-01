use super::*;

pub struct SimpleRandomQsc {
    desired_quorum_set_size: usize,
    desired_threshold: usize,
    adapt_until_satisfied: bool,
}
impl SimpleRandomQsc {
    pub fn new(desired_quorum_set_size: usize, desired_threshold: usize) -> Self {
        if desired_threshold > desired_quorum_set_size {
            warn!(
                "Desired threshold higher than desired quorum set size; \
                 will be set to equal quorum set size."
            );
        }
        SimpleRandomQsc {
            desired_quorum_set_size,
            desired_threshold,
            adapt_until_satisfied: true,
        }
    }
    pub fn never_adapt(mut self) -> Self {
        self.adapt_until_satisfied = false;
        self
    }
}
impl QuorumSetConfigurator for SimpleRandomQsc {
    fn configure(&self, node_id: NodeId, fbas: &mut Fbas) -> ChangeEffect {
        let n = fbas.nodes.len();
        let existing_quorum_set = &mut fbas.nodes[node_id].quorum_set;

        if (self.adapt_until_satisfied
            && (existing_quorum_set.validators.len() < self.desired_quorum_set_size))
            || *existing_quorum_set == QuorumSet::new()
        {
            // we are not satisfied or it is an empty quorum set
            let quorum_set_size = cmp::min(self.desired_quorum_set_size, n);
            let threshold = cmp::min(quorum_set_size, self.desired_threshold);

            let used_nodes: BitSet<NodeId> =
                existing_quorum_set.validators.iter().copied().collect();
            let available_nodes: Vec<NodeId> =
                (0..n).filter(|&x| !used_nodes.contains(x)).collect();

            let new_validators: Vec<NodeId> = available_nodes
                .choose_multiple(&mut thread_rng(), quorum_set_size)
                .copied()
                .collect();

            existing_quorum_set.validators.extend(new_validators);
            existing_quorum_set.threshold = threshold;

            Change
        } else {
            NoChange
        }
    }
}

#[cfg(test)]
mod tests {
    use super::monitors::*;
    use super::*;

    #[test]
    fn simple_random_qsc_makes_a_quorum() {
        let mut simulator = Simulator::new(
            Fbas::new(),
            Rc::new(SimpleRandomQsc::new(2, 1)),
            Rc::new(DummyMonitor),
        );
        simulator.simulate_growth(3);
        assert!(simulator.fbas.is_quorum(&bitset![0, 1, 2]));
    }

    #[test]
    fn simple_random_qsc_adapts_until_satisfied() {
        let mut simulator_random = Simulator::new(
            Fbas::new(),
            Rc::new(SimpleRandomQsc::new(5, 3)),
            Rc::new(DummyMonitor),
        );
        let mut simulator_safe = Simulator::new(
            Fbas::new(),
            Rc::new(SuperSafeQsc::new()),
            Rc::new(DummyMonitor),
        );
        simulator_random.simulate_growth(2);
        simulator_safe.simulate_growth(2);

        assert!(simulator_random.fbas.is_quorum(&bitset![0, 1]));

        simulator_random.simulate_growth(10);
        simulator_safe.simulate_growth(10);

        assert_ne!(simulator_safe.fbas, simulator_random.fbas);
        assert!(!simulator_random.fbas.is_quorum(&bitset![0, 1]));
    }

    #[test]
    fn simple_random_qsc_is_random() {
        let mut simulator_random_1 = Simulator::new(
            Fbas::new(),
            Rc::new(SimpleRandomQsc::new(5, 3)),
            Rc::new(DummyMonitor),
        );
        let mut simulator_random_2 = simulator_random_1.clone();
        simulator_random_1.simulate_growth(23);
        simulator_random_2.simulate_growth(23);

        assert_ne!(simulator_random_1.fbas, simulator_random_2.fbas);
    }
}