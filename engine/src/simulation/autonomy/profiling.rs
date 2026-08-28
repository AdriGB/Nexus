//! Optional measurements for the canonical autonomy execution path.

pub(super) const PROFILE_SAMPLE_RATE: usize = 4;

#[derive(Clone, Debug, Default)]
pub(crate) struct AutonomyProfile {
    pub work: crate::simulation::WorkCounters,
    pub state: crate::simulation::StateGauges,
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
}

pub(in crate::simulation) fn should_profile_entity(index: usize) -> bool {
    index.is_multiple_of(PROFILE_SAMPLE_RATE)
}
