use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MODEL_JSON: &str = include_str!("../../contracts/sync-fault-model-v1.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Model {
    model_version: String,
    generator: Generator,
    execution: Execution,
    operations: Vec<String>,
    faults: Vec<String>,
    invariants: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Generator {
    algorithm: String,
    base_seed: u64,
    steps_per_sequence: usize,
    devices: usize,
    entities: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Execution {
    environment_variable: String,
    pull_request_sequences: usize,
    nightly_sequences: usize,
}

struct XorShift64Star(u64);

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn pick(&mut self, upper: usize) -> usize {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        (value.wrapping_mul(2_685_821_657_736_338_717) % upper as u64) as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entity {
    version: u64,
    tombstone: bool,
    value: String,
    history: Vec<History>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct History {
    tombstone: bool,
    kind: String,
}

#[derive(Clone, Debug)]
struct Mutation {
    operation_id: String,
    device: usize,
    entity_id: String,
    kind: String,
    base_version: u64,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Response {
    status: String,
    version: u64,
    reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct View {
    version: u64,
    tombstone: bool,
    value: String,
}

struct Device {
    known_versions: BTreeMap<String, u64>,
    views: BTreeMap<String, View>,
    pending: BTreeMap<String, String>,
}

struct State {
    entities: BTreeMap<String, Entity>,
    responses: BTreeMap<String, Response>,
    devices: Vec<Device>,
    queue: VecDeque<Mutation>,
    terminal: BTreeMap<String, Response>,
    trace: Vec<Value>,
    faults_seen: BTreeSet<String>,
}

fn load_model() -> Model {
    let model: Model = serde_json::from_str(MODEL_JSON).expect("valid fault model contract");
    assert_eq!(model.model_version, "sync-fault-model/v1");
    assert_eq!(model.generator.algorithm, "xorshift64star");
    assert!(model.execution.pull_request_sequences >= 1_000);
    assert!(model.execution.nightly_sequences >= 10_000);
    assert_eq!(
        model.invariants.iter().cloned().collect::<BTreeSet<_>>(),
        [
            "eventualConvergence",
            "exactIdempotentReplay",
            "explicitStaleConflict",
            "noSilentOverwrite",
            "noSilentResurrection",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    model
}

fn view(entity: &Entity) -> View {
    View {
        version: entity.version,
        tombstone: entity.tombstone,
        value: entity.value.clone(),
    }
}

fn apply(state: &mut State, mutation: &Mutation) -> Response {
    if let Some(response) = state.responses.get(&mutation.operation_id) {
        return response.clone();
    }
    let entity = state.entities.get_mut(&mutation.entity_id).unwrap();
    let response = if mutation.base_version != entity.version {
        Response {
            status: "conflict".into(),
            version: entity.version,
            reason: Some(if entity.tombstone {
                "tombstoneDivergence".into()
            } else {
                "divergentContent".into()
            }),
        }
    } else if entity.tombstone && mutation.kind != "restore" {
        Response {
            status: "conflict".into(),
            version: entity.version,
            reason: Some("tombstoneDivergence".into()),
        }
    } else if !entity.tombstone && mutation.kind == "restore" {
        Response {
            status: "conflict".into(),
            version: entity.version,
            reason: Some("invalidRestore".into()),
        }
    } else {
        entity.version += 1;
        entity.tombstone = mutation.kind == "delete";
        if mutation.kind != "delete" {
            entity.value.clone_from(&mutation.value);
        }
        entity.history.push(History {
            tombstone: entity.tombstone,
            kind: mutation.kind.clone(),
        });
        Response {
            status: "applied".into(),
            version: entity.version,
            reason: None,
        }
    };
    state
        .responses
        .insert(mutation.operation_id.clone(), response.clone());
    response
}

fn settle(state: &mut State, mutation: &Mutation, response: Response) {
    let device = &mut state.devices[mutation.device];
    if device.pending.get(&mutation.entity_id) == Some(&mutation.operation_id) {
        device.pending.remove(&mutation.entity_id);
        let cloud = state.entities.get(&mutation.entity_id).unwrap();
        device
            .known_versions
            .insert(mutation.entity_id.clone(), cloud.version);
        device.views.insert(mutation.entity_id.clone(), view(cloud));
    }
    state
        .terminal
        .insert(mutation.operation_id.clone(), response);
}

fn deliver(state: &mut State, index: usize, should_settle: bool) -> Response {
    let mutation = state.queue[index].clone();
    let response = apply(state, &mutation);
    if should_settle {
        state.queue.remove(index);
        settle(state, &mutation, response.clone());
    }
    response
}

fn snapshot(state: &mut State, device_index: usize) {
    let device = &mut state.devices[device_index];
    for (entity_id, cloud) in &state.entities {
        if device.pending.contains_key(entity_id) {
            continue;
        }
        device
            .known_versions
            .insert(entity_id.clone(), cloud.version);
        device.views.insert(entity_id.clone(), view(cloud));
    }
}

fn run_sequence(seed: u64, model: &Model) -> State {
    let mut rng = XorShift64Star::new(seed);
    let entities: BTreeMap<_, _> = (0..model.generator.entities)
        .map(|index| {
            (
                format!("entity-{index}"),
                Entity {
                    version: 1,
                    tombstone: false,
                    value: format!("initial-{index}"),
                    history: vec![History {
                        tombstone: false,
                        kind: "create".into(),
                    }],
                },
            )
        })
        .collect();
    let devices = (0..model.generator.devices)
        .map(|_| Device {
            known_versions: entities.keys().map(|key| (key.clone(), 1)).collect(),
            views: entities
                .iter()
                .map(|(key, entity)| (key.clone(), view(entity)))
                .collect(),
            pending: BTreeMap::new(),
        })
        .collect();
    let mut state = State {
        entities,
        responses: BTreeMap::new(),
        devices,
        queue: VecDeque::new(),
        terminal: BTreeMap::new(),
        trace: Vec::new(),
        faults_seen: BTreeSet::new(),
    };

    for step in 0..model.generator.steps_per_sequence {
        let device_index = rng.pick(model.generator.devices);
        let entity_id = format!("entity-{}", rng.pick(model.generator.entities));
        let device = &mut state.devices[device_index];
        if !device.pending.contains_key(&entity_id) {
            let current = device.views.get(&entity_id).unwrap();
            let requested = &model.operations[rng.pick(model.operations.len())];
            let kind = if current.tombstone {
                if requested == "delete" {
                    "delete"
                } else {
                    "restore"
                }
            } else if requested == "restore" {
                "update"
            } else {
                requested
            };
            let mutation = Mutation {
                operation_id: format!("{seed}-{step}"),
                device: device_index,
                entity_id: entity_id.clone(),
                kind: kind.into(),
                base_version: device.known_versions[&entity_id],
                value: format!("value-{seed}-{step}"),
            };
            device
                .pending
                .insert(entity_id.clone(), mutation.operation_id.clone());
            device.views.insert(
                entity_id.clone(),
                View {
                    version: current.version,
                    tombstone: kind == "delete",
                    value: if kind == "delete" {
                        current.value.clone()
                    } else {
                        mutation.value.clone()
                    },
                },
            );
            state.queue.push_back(mutation);
        }

        let fault = &model.faults[rng.pick(model.faults.len())];
        state.faults_seen.insert(fault.clone());
        let mut event = json!({"step": step, "fault": fault, "queueDepth": state.queue.len()});
        let response = match (state.queue.is_empty(), fault.as_str()) {
            (false, "none") => Some(deliver(&mut state, 0, true)),
            (false, "timeoutAfterCommit") => Some(deliver(&mut state, 0, false)),
            (false, "duplicateDelivery") => {
                let first = deliver(&mut state, 0, false);
                let replay = deliver(&mut state, 0, false);
                assert_eq!(replay, first);
                let mutation = state.queue.pop_front().unwrap();
                settle(&mut state, &mutation, replay.clone());
                Some(replay)
            }
            (false, "outOfOrderDelivery") => {
                let index = state.queue.len() - 1;
                Some(deliver(&mut state, index, true))
            }
            (_, "cursorExpired") => {
                snapshot(&mut state, device_index);
                None
            }
            _ => None,
        };
        if let Some(response) = response {
            event["response"] = json!({
                "status": response.status,
                "version": response.version,
                "reason": response.reason,
            });
        }
        state.trace.push(event);
    }

    while let Some(mutation) = state.queue.front().cloned() {
        let first = deliver(&mut state, 0, false);
        let replay = deliver(&mut state, 0, false);
        assert_eq!(replay, first);
        state.queue.pop_front();
        settle(&mut state, &mutation, replay);
    }
    for device_index in 0..model.generator.devices {
        snapshot(&mut state, device_index);
    }
    state
}

fn state_diff(state: &State) -> Value {
    let cloud: BTreeMap<_, _> = state
        .entities
        .iter()
        .map(|(key, entity)| {
            (
                key,
                json!({"version": entity.version, "tombstone": entity.tombstone, "value": entity.value}),
            )
        })
        .collect();
    let devices: Vec<_> = state
        .devices
        .iter()
        .map(|device| {
            device
                .views
                .iter()
                .map(|(key, view)| {
                    (
                        key,
                        json!({"version": view.version, "tombstone": view.tombstone, "value": view.value}),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect();
    json!({"cloud": cloud, "devices": devices})
}

fn assert_invariants(seed: u64, state: &State) -> Result<(), String> {
    let mut errors = Vec::new();
    let diff = state_diff(state);
    for (index, device) in state.devices.iter().enumerate() {
        if json!(device
            .views
            .iter()
            .map(|(key, view)| (
                key,
                json!({"version": view.version, "tombstone": view.tombstone, "value": view.value})
            ))
            .collect::<BTreeMap<_, _>>())
            != diff["cloud"]
        {
            errors.push(format!("device {index} did not converge"));
        }
        if !device.pending.is_empty() {
            errors.push(format!("device {index} retained pending candidates"));
        }
    }
    if !state.queue.is_empty() {
        errors.push("durable queue did not settle".into());
    }
    if state.responses != state.terminal {
        errors.push("operation replay or terminal response mismatch".into());
    }
    for (entity_id, entity) in &state.entities {
        if entity.version as usize != entity.history.len() {
            errors.push(format!("semantic version gap for {entity_id}"));
        }
        for pair in entity.history.windows(2) {
            if pair[0].tombstone && !pair[1].tombstone && pair[1].kind != "restore" {
                errors.push(format!("silent resurrection for {entity_id}"));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "sync fault model invariant failure: {}",
            json!({"seed": seed, "errors": errors, "operationsAndFaults": state.trace, "stateDiff": diff})
        ))
    }
}

#[test]
fn fixed_seed_randomized_sequences_preserve_sync_safety() {
    let model = load_model();
    let count = std::env::var(&model.execution.environment_variable)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(model.execution.pull_request_sequences);
    assert!(count >= model.execution.pull_request_sequences);
    let mut faults_seen = BTreeSet::new();
    for offset in 0..count {
        let seed = model.generator.base_seed + offset as u64;
        let state = run_sequence(seed, &model);
        assert_invariants(seed, &state).unwrap_or_else(|diagnostic| panic!("{diagnostic}"));
        faults_seen.extend(state.faults_seen);
    }
    assert_eq!(
        faults_seen,
        model.faults.iter().cloned().collect::<BTreeSet<_>>()
    );
}

#[test]
fn seed_is_reproducible_and_failure_diagnostic_is_actionable() {
    let model = load_model();
    let seed = model.generator.base_seed + 17;
    let first = run_sequence(seed, &model);
    let second = run_sequence(seed, &model);
    assert_eq!(state_diff(&first), state_diff(&second));

    let mut corrupted = first;
    corrupted.devices[0]
        .views
        .get_mut("entity-0")
        .unwrap()
        .version = 999;
    let diagnostic = assert_invariants(seed, &corrupted).unwrap_err();
    assert!(diagnostic.contains(&format!("\"seed\":{seed}")));
    assert!(diagnostic.contains("operationsAndFaults"));
    assert!(diagnostic.contains("stateDiff"));
}
