// Shared by the two offline pages. Input phases and tasks are in Markdown order.
function roadmapMoves(phases, definitions, taskDeps) {
  const byId = new Map(Object.values(definitions).map(phase => [phase.id, phase]));
  const visible = phases.filter(phase => byId.has(phase.id));
  const done = new Set(visible.flatMap(phase => phase.tasks.filter(t => t.state === 'done').map(t => t.id)));
  const started = new Set();
  const completed = new Set();
  const current = [];
  for (const phase of visible) {
    const active = phase.tasks.filter(t => t.state === 'cur');
    current.push(...active.map(t => t.id));
    const required = phase.tasks.filter(t => !t.optional);
    if (active.length || required.some(t => done.has(t.id))) started.add(phase.id);
    const requiredForCompletion = required.length ? required : phase.tasks;
    if (requiredForCompletion.length && requiredForCompletion.every(t => done.has(t.id))) completed.add(phase.id);
  }

  // These phases need a specific R2 milestone and can overlap the rest of R2.
  const r2Milestone = { R3: 'R2.1', R7: 'R2.3', R8: 'R2.4' };
  const missingParents = phase => {
    if (started.has(phase.id)) return [];
    return byId.get(phase.id).deps.flatMap(key => {
      if (key === 'R2' && r2Milestone[phase.id]) {
        const milestone = r2Milestone[phase.id];
        return done.has(milestone) ? [] : [milestone];
      }
      const parent = definitions[key];
      return completed.has(parent.id) || (parent.id === '0.4.2' && parent.cls === 'done') ? [] : [parent.id];
    });
  };

  const resumed = [], continuing = [], fresh = [], optional = [];
  let blocked;
  for (const phase of visible) {
    if (phase.tasks.some(t => t.state === 'cur')) continue;
    const pending = phase.tasks.filter(t => t.state === 'todo');
    // Choose the phase head before checking its dependencies: a blocked head holds the queue.
    const task = pending.find(t => !t.optional) || pending[0];
    if (!task) continue;
    const deps = taskDeps[task.id] || [];
    const waits = missingParents(phase).concat(deps.filter(id => !done.has(id)));
    if (waits.length) {
      if (!blocked || (blocked.task.optional && !task.optional)) blocked = { task, waits };
      continue;
    }

    if (task.optional) optional.push(task.id);
    else if (!started.has(phase.id)) fresh.push(task.id);
    else if (deps.length) resumed.push(task.id);
    else continuing.push(task.id);
  }

  // Resume a started phase at its cross-phase gate before other independent work.
  const next = resumed.concat(continuing);
  if (!next.length && !current.length) next.push(...(fresh.length ? fresh : optional).slice(0, 1));
  return { current, next, blocked };
}
