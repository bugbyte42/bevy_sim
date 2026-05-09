use crate::{
    CommodityId, FacilityState, Inventory, Recipe, RecipeBook, RecipeId, SimWorld, Stack,
    TickEvent, TransportEdge, TransportNodeId, TransportNodeState, TransportOrder,
};

fn wood() -> CommodityId {
    CommodityId::from("resource.wood")
}

fn plank() -> CommodityId {
    CommodityId::from("component.plank")
}

fn simple_recipe() -> Recipe {
    Recipe {
        id: RecipeId::from("recipe.saw_plank.v1"),
        inputs: vec![Stack::new(wood(), 2.0)],
        outputs: vec![Stack::new(plank(), 1.0)],
        byproducts: vec![],
        facility_tags: vec!["sawmill".to_string()],
        duration_ticks: 2,
    }
}

#[test]
fn inventory_adds_and_removes_quantities() {
    let mut inventory = Inventory::new();
    inventory.add(&wood(), 5.0).unwrap();
    inventory.remove(&wood(), 2.0).unwrap();

    assert_eq!(inventory.get(&wood()), 3.0);
    assert!(inventory.remove(&wood(), 4.0).is_err());
}

#[test]
fn insufficient_inputs_block_recipe_progress() {
    let mut world = SimWorld::new(RecipeBook::new([simple_recipe()]));
    world.add_facility(FacilityState::new(
        "facility.1",
        "facility.sawmill.tier1",
        Some(RecipeId::from("recipe.saw_plank.v1")),
    ));

    let report = world.tick();

    assert!(
        report
            .events
            .iter()
            .any(|event| matches!(event, TickEvent::FacilityBlocked { .. }))
    );
    assert_eq!(
        world
            .facilities
            .get(&crate::FacilityId::from("facility.1"))
            .unwrap()
            .progress_ticks,
        0
    );
}

#[test]
fn recipe_completes_after_multiple_ticks() {
    let mut world = SimWorld::new(RecipeBook::new([simple_recipe()]));
    world.global_inventory.add(&wood(), 2.0).unwrap();
    world.add_facility(FacilityState::new(
        "facility.1",
        "facility.sawmill.tier1",
        Some(RecipeId::from("recipe.saw_plank.v1")),
    ));

    let first = world.tick();
    assert!(first.events.iter().any(|event| matches!(
        event,
        TickEvent::FacilityProgressed {
            progress_ticks: 1,
            ..
        }
    )));
    assert_eq!(world.global_inventory.get(&plank()), 0.0);

    let second = world.tick();
    assert!(
        second
            .events
            .iter()
            .any(|event| matches!(event, TickEvent::RecipeCompleted { .. }))
    );
    assert_eq!(world.global_inventory.get(&wood()), 0.0);
    assert_eq!(world.global_inventory.get(&plank()), 1.0);
    assert_eq!(second.ledger.consumed_qty(&wood()), 2.0);
    assert_eq!(second.ledger.produced_qty(&plank()), 1.0);
}

#[test]
fn blocked_inputs_are_recorded_as_blocked_demand() {
    let mut world = SimWorld::new(RecipeBook::new([simple_recipe()]));
    world.global_inventory.add(&wood(), 0.5).unwrap();
    world.add_facility(FacilityState::new(
        "facility.1",
        "facility.sawmill.tier1",
        Some(RecipeId::from("recipe.saw_plank.v1")),
    ));

    let report = world.tick();

    assert_eq!(report.ledger.blocked_demand_qty(&wood()), 1.5);
}

#[test]
fn repeated_ticks_are_deterministic() {
    fn run() -> Vec<TickEvent> {
        let mut world = SimWorld::new(RecipeBook::new([simple_recipe()]));
        world.global_inventory.add(&wood(), 4.0).unwrap();
        world.add_facility(FacilityState::new(
            "facility.1",
            "facility.sawmill.tier1",
            Some(RecipeId::from("recipe.saw_plank.v1")),
        ));

        let mut events = Vec::new();
        for _ in 0..4 {
            events.extend(world.tick().events);
        }
        events
    }

    assert_eq!(run(), run());
}

#[test]
fn recipe_graph_reports_dependency_links_and_blockers() {
    let book = RecipeBook::new([simple_recipe()]);
    let links = book.links_for(&wood());
    let blockers =
        book.blocked_reasons_for(&RecipeId::from("recipe.saw_plank.v1"), &Inventory::new());

    assert_eq!(
        links.required_by,
        vec![RecipeId::from("recipe.saw_plank.v1")]
    );
    assert_eq!(links.produced_by, Vec::<RecipeId>::new());
    assert_eq!(blockers.len(), 1);
}

#[test]
fn transport_edges_move_limited_quantities() {
    let mut world = SimWorld::new(RecipeBook::default());
    let mine = TransportNodeId::from("node.mine");
    let settlement = TransportNodeId::from("node.settlement");
    world.add_node(TransportNodeState::new(mine.clone()));
    world.add_node(TransportNodeState::new(settlement.clone()));
    world
        .node_inventory_mut(&mine)
        .unwrap()
        .add(&wood(), 10.0)
        .unwrap();
    world.add_edge(TransportEdge::new(
        "edge.mine_to_settlement",
        mine,
        settlement.clone(),
        2.0,
        1.0,
    ));
    world.add_transport_order(TransportOrder::new(
        "order.wood",
        "edge.mine_to_settlement",
        wood(),
        10.0,
        5.0,
    ));

    let report = world.tick();

    assert!(report.events.iter().any(|event| matches!(
        event,
        TickEvent::TransportMoved {
            qty,
            capacity_limited: true,
            ..
        } if (*qty - 2.0).abs() < crate::EPSILON
    )));
    assert_eq!(world.node_inventory(&settlement).unwrap().get(&wood()), 2.0);
    assert_eq!(report.ledger.moved_out_qty(&wood()), 2.0);
    assert_eq!(report.ledger.moved_in_qty(&wood()), 2.0);
}

#[test]
fn blocked_transport_records_destination_need() {
    let mut world = SimWorld::new(RecipeBook::default());
    let mine = TransportNodeId::from("node.mine");
    let settlement = TransportNodeId::from("node.settlement");
    world.add_node(TransportNodeState::new(mine.clone()));
    world.add_node(TransportNodeState::new(settlement));
    world.add_edge(TransportEdge::new(
        "edge.mine_to_settlement",
        mine,
        "node.settlement",
        2.0,
        1.0,
    ));
    world.add_transport_order(TransportOrder::new(
        "order.wood",
        "edge.mine_to_settlement",
        wood(),
        10.0,
        5.0,
    ));

    let report = world.tick();

    assert!(
        report
            .events
            .iter()
            .any(|event| matches!(event, TickEvent::TransportBlocked { .. }))
    );
    assert_eq!(report.ledger.blocked_demand_qty(&wood()), 10.0);
}
