//! Optional measurements for the canonical autonomy execution path.

pub(super) const PROFILE_SAMPLE_RATE: usize = 4;

/// Coste de las ocho llamadas que `Simulation::execute_autonomy` hace después
/// del bucle por-entidad y del social pass.
///
/// Esas dos pasadas ya se cronometran con `entity_pass_us` y `social_us`, pero
/// el trabajo posterior quedaba fuera de ambos y era el 30% del paso a 10k sin
/// atribuir (#195). Se mide cada llamada por separado porque los volúmenes que
/// reciben son muy distintos entre sí.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PostPassProfile {
    pub resource_discoveries_us: u64,
    pub entity_encounters_us: u64,
    pub food_consumptions_us: u64,
    pub social_interactions_us: u64,
    pub food_share_us: u64,
    pub household_deposit_us: u64,
    pub household_withdraw_us: u64,
    pub household_conflict_us: u64,
}

impl PostPassProfile {
    pub(crate) fn total_us(&self) -> u64 {
        self.resource_discoveries_us
            .saturating_add(self.entity_encounters_us)
            .saturating_add(self.food_consumptions_us)
            .saturating_add(self.social_interactions_us)
            .saturating_add(self.food_share_us)
            .saturating_add(self.household_deposit_us)
            .saturating_add(self.household_withdraw_us)
            .saturating_add(self.household_conflict_us)
    }

    /// Sólo la usa `benchmarking`, que está tras `#[cfg(feature = "benchmarks")]`.
    #[cfg(feature = "benchmarks")]
    pub(crate) fn accumulate(&mut self, other: &Self) {
        self.resource_discoveries_us = self
            .resource_discoveries_us
            .saturating_add(other.resource_discoveries_us);
        self.entity_encounters_us = self
            .entity_encounters_us
            .saturating_add(other.entity_encounters_us);
        self.food_consumptions_us = self
            .food_consumptions_us
            .saturating_add(other.food_consumptions_us);
        self.social_interactions_us = self
            .social_interactions_us
            .saturating_add(other.social_interactions_us);
        self.food_share_us = self.food_share_us.saturating_add(other.food_share_us);
        self.household_deposit_us = self
            .household_deposit_us
            .saturating_add(other.household_deposit_us);
        self.household_withdraw_us = self
            .household_withdraw_us
            .saturating_add(other.household_withdraw_us);
        self.household_conflict_us = self
            .household_conflict_us
            .saturating_add(other.household_conflict_us);
    }
}

/// The per-entity pass, decomposed into the blocks that carry signal.
///
/// Full population, no sampling — the timers run for every entity, every tick.
/// These four are the ones that exceed 5% of `entity_pass_us` in at least one of
/// the eight scenarios (#207). `execute_current_action` and
/// `prune_expired_grief` never do, so they are deliberately left outside and
/// land in the residual, which is `entity_pass_us - total_ns()`.
///
/// Nanoseconds rather than the microseconds the rest of the profile uses. These
/// timers fire once per entity, so a block costing 0.6 µs at 100 entities would
/// truncate to zero on every single entity and the measurement would vanish.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct EntityPassBreakdown {
    /// `perception::perceive_entities`: spatial lookup of nearby entities.
    pub perceive_entities_ns: u64,
    /// Plan invalidation: obsolete food plans, dependents, migration, interrupts.
    pub plan_validation_ns: u64,
    /// `decision::evaluate_goals` plus `decision::plan_goal`, including
    /// pathfinding. Only runs for entities with no current action.
    pub planning_ns: u64,
    /// `reconcile_resource_memory` plus `scan_visible_resources`.
    pub resource_memory_ns: u64,
}

impl EntityPassBreakdown {
    /// Adds another tick's worth of per-entity timings. Used by
    /// `run_autonomy`, which drains a per-tick accumulator into the profile, and
    /// by `benchmarking`, which sums the profiles of the profiled ticks.
    pub(crate) fn accumulate(&mut self, other: &Self) {
        self.perceive_entities_ns = self
            .perceive_entities_ns
            .saturating_add(other.perceive_entities_ns);
        self.plan_validation_ns = self
            .plan_validation_ns
            .saturating_add(other.plan_validation_ns);
        self.planning_ns = self.planning_ns.saturating_add(other.planning_ns);
        self.resource_memory_ns = self
            .resource_memory_ns
            .saturating_add(other.resource_memory_ns);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AutonomyProfile {
    pub work: crate::simulation::WorkCounters,
    pub state: crate::simulation::StateGauges,
    pub post_pass: PostPassProfile,
    pub resource_perception_us: u64,
    pub entity_perception_us: u64,
    pub plan_validation_us: u64,
    pub planning_us: u64,
    pub action_us: u64,
    pub sampled_entities: u32,
    pub planned_entities: u32,
    pub urgent_interrupts: u32,
    pub memory_reconciliation_us: u64,
    pub visible_scan_us: u64,
    pub sampled_known_resources_total: u32,
    pub sampled_known_resources_max: u32,
    pub visible_resources_seen: u32,
    pub social_us: u64,
    /// Bucle por-entidad completo, medido sobre **toda** la población.
    ///
    /// Es el equivalente de `social_us` para la otra mitad de la fase: un único
    /// cronómetro sin filtrar, así que no depende del muestreo de
    /// `PROFILE_SAMPLE_RATE` (#191) ni de su extrapolación.
    ///
    /// Ojo: NO es comparable con `summary.autonomy.mean_us`. Ese número sale de
    /// la pasada de fases, donde los temporizadores por entidad están apagados;
    /// éste sale de la pasada perfilada, que es más lenta. Comparar los dos
    /// mezcla medidas de mundos distintos. El denominador correcto es
    /// `step_total_us`.
    pub entity_pass_us: u64,
    /// Muro del paso completo, medido sólo alrededor de `execute_step`.
    ///
    /// Excluye `state_gauges()`, que la pasada perfilada calcula después y que no
    /// forma parte del paso de simulación. Es el denominador que convierte
    /// `social_us + entity_pass_us` en una fracción: los tres se cronometran
    /// dentro de la misma pasada.
    pub step_total_us: u64,
    /// Descomposición del bucle por-entidad, sobre **toda** la población.
    ///
    /// Es lo que `entity_pass_us` no daba: ese número decía cuánto costaba la
    /// pasada, no de qué estaba hecha. El residuo sin atribuir es
    /// `entity_pass_us * 1000 - entity_pass.total_ns()` y (#207) quedó por debajo
    /// del 3% a 1.000 y 10.000 entidades; lo que quepa ahí son los dos bloques
    /// excluidos a propósito más el coste de los propios cronómetros.
    pub entity_pass: EntityPassBreakdown,
}

pub(in crate::simulation) fn should_profile_entity(index: usize) -> bool {
    index.is_multiple_of(PROFILE_SAMPLE_RATE)
}
