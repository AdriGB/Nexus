import { state } from "../state";
import type { PopulationStats } from "../types";

export function syncPopulationStats(): void {
  if (!state.world) return;
  const stats: PopulationStats = JSON.parse(state.world.population_stats());
  const values: Record<string, string> = {
    "population-count": stats.population.toLocaleString(),
    "population-females": stats.females.toLocaleString(),
    "population-males": stats.males.toLocaleString(),
    "population-pregnant": stats.pregnant.toLocaleString(),
    "population-births": stats.births.toLocaleString(),
    "population-deaths": stats.deaths.toLocaleString(),
    "population-hungry": stats.hungry.toLocaleString(),
    "population-seeking": stats.seeking_food.toLocaleString(),
    "population-average-hunger": `${stats.average_hunger.toFixed(1)}%`,
    "population-food-consumed": stats.food_consumed.toLocaleString(),
  };
  for (const [id, value] of Object.entries(values)) {
    document.getElementById(id)!.textContent = value;
  }
}
