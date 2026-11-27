//! Native module data retained only inside one transactional schedule tick.
use crate::GameState;
use bevy_ecs::{resource::Resource, world::World};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Resource)]
pub(crate) struct NativeTickValue<T: Send + Sync + 'static>(pub T);

type Conversion = Arc<dyn Fn(&mut GameState, &mut World) -> Result<(), String> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct TickDataContract {
    pub(crate) type_id: std::any::TypeId,
    read: Conversion,
    write: Conversion,
}

impl TickDataContract {
    pub(crate) fn new<T: Send + Sync + 'static>(
        read: fn(&GameState) -> Result<T, String>,
        write: fn(T, &mut GameState) -> Result<(), String>,
    ) -> Self {
        Self {
            type_id: std::any::TypeId::of::<T>(),
            read: Arc::new(move |game, world| {
                world.insert_resource(NativeTickValue(read(game)?));
                Ok(())
            }),
            write: Arc::new(move |game, world| {
                let value = world
                    .remove_resource::<NativeTickValue<T>>()
                    .ok_or("Missing native tick data")?;
                write(value.0, game)
            }),
        }
    }
}

/// No value survives an accepted or rejected tick. A foreign system always
/// receives portable data, and its edits are decoded before native reuse.
#[derive(Default)]
pub(crate) struct TickData {
    pub(crate) world: Option<World>,
    contracts: BTreeMap<String, TickDataContract>,
    active: BTreeSet<String>,
}

impl TickData {
    pub(crate) fn new(contracts: BTreeMap<String, TickDataContract>) -> Self {
        Self {
            contracts,
            ..Default::default()
        }
    }

    pub(crate) fn enter(&mut self, id: &str, game: &mut GameState) -> Result<(), String> {
        if !self.active.contains(id) {
            let contract = self
                .contracts
                .get(id)
                .ok_or_else(|| format!("No native tick-data contract registered for '{id}'"))?;
            (contract.read)(game, self.world.get_or_insert_with(World::new))?;
            self.active.insert(id.into());
        }
        Ok(())
    }

    pub(crate) fn boundary(
        &mut self,
        participant: Option<&str>,
        game: &mut GameState,
    ) -> Result<(), String> {
        let leaving: Vec<_> = self
            .active
            .iter()
            .filter(|id| Some(id.as_str()) != participant)
            .cloned()
            .collect();
        for id in leaving {
            self.active.remove(&id);
            (self.contracts[&id].write)(
                game,
                self.world.as_mut().expect("Active tick data has a world"),
            )?;
        }
        if let Some(id) = participant {
            self.enter(id, game)?;
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self, game: &mut GameState) -> Result<BTreeSet<String>, String> {
        let native = self.active.clone();
        self.boundary(None, game)?;
        Ok(native)
    }
}
