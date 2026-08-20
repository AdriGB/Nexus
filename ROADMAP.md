# NEXUS Roadmap

## Visión

Nexus es un laboratorio de sociedades emergentes.

El usuario crea un mundo, establece sus condiciones y define reglas sociales, económicas o culturales. Individuos autónomos viven bajo esas condiciones, forman relaciones, construyen grupos y producen una historia que puede observarse, compararse y explicarse causalmente.

Principios:

* Los individuos solo actúan con información que conocen.
* Los resultados deben emerger de mecanismos, no de historias prefabricadas.
* Toda decisión importante debe poder explicarse.
* Una misma semilla debe producir el mismo resultado.
* Los sistemas deben permanecer inactivos hasta que tengan algo relevante que procesar.
* Cada fase debe entregar una pequeña historia emergente completa.

---

# Phase 1 — World Foundation ✅

## Phase 1.1 — World Viewer ✅

* [x] Generación procedural determinista
* [x] Terrenos y biomas iniciales
* [x] Cámara, minimapa e inspector
* [x] Persistencia de parámetros
* [x] Importación y exportación del mundo

## Phase 1.2 — Geography

* [x] Detección de regiones
* [x] Conectividad geográfica
* [ ] Refinamiento climático de biomas
* [ ] Ríos y cuencas
* [ ] Cambios ambientales a largo plazo

Las funciones pendientes de geografía quedan pospuestas hasta que influyan directamente sobre la simulación.

## Phase 1.3 — GPU Renderer ✅

* [x] Renderizador wgpu/WebGPU
* [x] Textura mundial
* [x] Cámara mediante uniforms
* [x] Overlays de selección
* [x] Entidades instanciadas
* [x] Canvas 2D como fallback congelado
* [x] Validación WGSL en CI

## Phase 1.4 — Traversal ✅

* [x] Costos de movimiento
* [x] Walkability
* [x] A* de ocho direcciones
* [x] Prevención de corner cutting
* [x] Workspace reutilizable
* [x] Rutas visuales
* [x] Movimiento tile por tile
* [x] Tests de pathfinding

---

# Phase 2 — Living Individuals

## Phase 2.1 — Resources ✅

* [x] Capa de recursos independiente
* [x] Comida, madera, piedra y hierro
* [x] Distribución determinista
* [x] Cantidades finitas
* [x] Visualización de recursos
* [x] Consumo y agotamiento

## Phase 2.2 — Simulation Clock ✅

* [x] Reloj determinista
* [x] Un tick representa una hora
* [x] Pausa, reproducción, velocidad y avance manual
* [x] Simulación independiente del renderizado
* [x] Procesamiento de ticks en bloques
* [x] Protección contra saltos temporales

## Phase 2.3 — Autonomous Entities ✅

* [x] Entidades persistentes
* [x] Percepción local
* [x] Memoria imperfecta
* [x] Utility AI
* [x] Goals persistentes
* [x] Planes y acciones
* [x] Comer, explorar, descansar y seguir
* [x] Memoria de recursos agotados o inaccesibles
* [x] Inspector cognitivo

## Phase 2.4 — Population ✅

* [x] Población determinista
* [x] Hambre y salud
* [x] Muerte por inanición
* [x] Competencia por recursos
* [x] Estadísticas demográficas
* [x] Índice espacial de entidades
* [x] Benchmarks de 100, 1.000 entidades

## Phase 2.5 — Biology ✅

* [x] Sexo biológico
* [x] Edad y etapas vitales
* [x] Esperanza de vida individual
* [x] Reproducción
* [x] Embarazo y gestación
* [x] Nacimiento
* [x] Postparto
* [x] Dependencia infantil
* [x] Cuidadores
* [x] Penalización de movilidad durante el embarazo
* [x] Muerte natural

## Phase 2.6 — Personality and Relationship Memory ✅

* [x] Personalidades deterministas
* [x] Curiosidad
* [x] Sociabilidad
* [x] Cooperación
* [x] Cautela
* [x] Persistencia
* [x] Personalidad aplicada a Utility AI
* [x] Memoria de individuos conocidos
* [x] Afinidad persistente

---

# Phase 2.7 — Social Interaction ✅

Objetivo: convertir las relaciones almacenadas en comportamiento visible.

* [x] Goal `Socialize`
* [x] Acción `ApproachEntity`
* [x] Acción `Interact`
* [x] Selección de interlocutor mediante:

  * [x] sociabilidad
  * [x] afinidad
  * [x] familiaridad
  * [x] distancia
  * [x] necesidades actuales
* [x] Resultado determinista de una interacción
* [x] Compatibilidad entre personalidades
* [x] Cambios positivos y negativos de afinidad
* [x] Cooldown social
* [x] Evitar individuos con afinidad negativa
* [X] Buscar individuos con afinidad positiva (desde memoria)
* [x] Decaimiento lento de relaciones abandonadas
* [x] Inspector de relaciones conocidas
* [x] Tests de formación y deterioro de afinidad
* [x] Reproducción influida por relaciones existentes

### Vertical de validación

Dos individuos se conocen, recuerdan sus encuentros, desarrollan afinidad y comienzan a buscarse voluntariamente. Una mala interacción también puede hacer que se eviten.

---

# Phase 2.8 — Events and Causality

Objetivo: registrar por qué sucede cada acontecimiento importante.

* [x] Modelo central `SimulationEvent`
* [x] Tick y ubicación del evento
* [x] Actor, objetivo y entidades relacionadas
* [x] Tipo de evento
* [x] Causa inmediata
* [ ] Eventos de:

  * [x] nacimiento
  * [x] muerte
  * [x] consumo
  * [x] descubrimiento
  * [x] encuentro
  * [x] interacción
  * [x] cambio significativo de afinidad
  * [ ] formación de pareja
  * [ ] separación
* [x] Buffer circular de eventos recientes
* [x] Historial resumido por entidad
* [x] Panel cronológico filtrable
* [x] Explicación de decisiones importantes
* [x] Exportación del historial
* [x] IDs causales entre eventos relacionados

### Vertical de validación

El usuario selecciona una relación y Nexus explica cuándo se conocieron, qué interacciones modificaron su afinidad y por qué actualmente se buscan o se evitan.

---

# Phase 2.9 — Material Survival

Objetivo: reemplazar el consumo directo por un ciclo económico mínimo.

## Inventories

* [x] Inventario personal
* [x] Capacidad de carga
* [x] Tipos y cantidades de objetos
* [ ] Transferencia entre entidades
* [x] Inspector de inventario

## Gathering

* [ ] Goal `AcquireResource`
* [ ] Acción `Gather`
* [ ] Duración de recolección
* [ ] Herramientas opcionales
* [ ] Agotamiento de depósitos
* [ ] Memoria de lugares productivos

## Food cycle

* [ ] Recolectar comida
* [ ] Transportar comida
* [ ] Consumir desde el inventario
* [ ] Compartir comida
* [ ] Alimentar dependientes
* [ ] Regeneración limitada de recursos renovables
* [ ] Estacionalidad básica futura

## Cooperation

* [ ] Cooperación influida por personalidad
* [ ] Afinidad influye en el reparto
* [ ] Rechazo a compartir
* [ ] Gratitud o resentimiento
* [ ] Eventos de ayuda y abandono

### Vertical de validación

Una población puede sobrevivir durante años mediante recolección, transporte y cooperación, pero puede colapsar por aislamiento, sobreexplotación o decisiones deficientes.

---

# Phase 2.10 — Families and Households

Objetivo: crear la primera estructura social superior al individuo.

## Kinship

* [ ] Madre y padre persistentes
* [ ] Hijos
* [ ] Hermanos
* [ ] Parejas
* [ ] Árbol familiar
* [ ] Relaciones de parentesco derivadas

## Households

* [ ] Formación de hogares
* [ ] Residencia compartida
* [ ] Miembros del hogar
* [ ] Almacén común
* [ ] Reparto de recursos
* [ ] Responsabilidad por dependientes
* [ ] Incorporación y abandono de miembros
* [ ] Disolución del hogar
* [ ] Herencia básica
* [ ] Estadísticas de hogares

## Relationship-driven behavior

* [ ] Buscar pareja o familiares
* [ ] Proteger dependientes
* [ ] Compartir según parentesco y afinidad
* [ ] Migrar junto al hogar
* [ ] Duelo por la muerte de personas cercanas
* [ ] Conflictos dentro del hogar

### Vertical de validación

Una pareja forma un hogar, tiene hijos, comparte recursos y atraviesa una escasez. Sus relaciones y personalidades determinan si el hogar coopera, se fragmenta o desaparece.

---

# Phase 2.11 — Persistence and Scale

Objetivo: garantizar que la creciente complejidad continúe siendo determinista, guardable y eficiente.

## Complete simulation persistence

* [ ] Guardar el estado completo de la simulación
* [ ] Entidades y mentes
* [ ] Relaciones
* [ ] Embarazos y parentescos
* [ ] Inventarios
* [ ] Hogares y grupos
* [ ] Recursos modificados
* [ ] Historial reciente
* [ ] Versión del formato
* [ ] Migraciones de partidas
* [ ] Hash determinista del estado
* [ ] Replay desde checkpoints

## Multi-rate simulation

* [ ] Movimiento por tick
* [ ] Percepción distribuida entre ticks
* [ ] Reevaluación de goals solo cuando sea necesaria
* [ ] Biología diaria cuando corresponda
* [ ] Hogares actualizados por eventos
* [ ] Economía diaria o semanal
* [ ] Cultura mensual o anual
* [ ] Procesamiento en bloques para periodos inactivos

## Performance budgets

* [ ] Profiling por sistema
* [ ] Tiempo promedio y máximo por tick
* [ ] Memoria por entidad
* [ ] Número de búsquedas A* por tick
* [ ] Presupuesto de decisiones
* [ ] Benchmarks automatizados:

  * [ ] 100 entidades
  * [ ] 1.000 entidades
  * [ ] 10.000 entidades
* [ ] Alertas de regresión en CI

### Objetivo de escala

* 100 entidades: simulación completamente detallada.
* 1.000 entidades: simulación detallada y fluida.
* 10.000 entidades: decisiones distribuidas y presupuestos por tick.
* Más de 10.000: resolución adaptativa o simulación agregada.

---

# Phase 3 — Rules and Consequences

## Phase 3.1 — Internal Rule Engine

* [ ] Representación serializable de reglas
* [ ] Condiciones
* [ ] Consecuencias
* [ ] Alcance:

  * [ ] individual
  * [ ] hogar
  * [ ] grupo
  * [ ] asentamiento
  * [ ] sociedad
* [ ] Prioridad entre reglas
* [ ] Reglas incompatibles
* [ ] Activación y desactivación
* [ ] Indexación de reglas por evento
* [ ] Evaluación únicamente ante eventos relevantes
* [ ] Explicación de cada regla aplicada

## Phase 3.2 — First Rule Vertical

Primera regla demostrativa:

> Los integrantes de un hogar deben compartir comida con sus dependientes.

* [ ] Detectar la situación relevante
* [ ] Aplicar la regla a una decisión
* [ ] Permitir obediencia o incumplimiento
* [ ] Registrar quién fue afectado
* [ ] Generar consecuencias relacionales
* [ ] Comparar el mundo con y sin la regla

## Phase 3.3 — Rule Editor

* [ ] Constructor condición → consecuencia
* [ ] Plantillas de reglas
* [ ] Validación
* [ ] Explicación en lenguaje natural
* [ ] Vista previa de entidades afectadas
* [ ] Importar y exportar reglas
* [ ] Conjuntos de reglas
* [ ] Modificar reglas durante la simulación

## Phase 3.4 — Scenario Comparison

* [ ] Clonar una simulación desde un checkpoint
* [ ] Modificar una sola condición
* [ ] Ejecutar escenarios en paralelo
* [ ] Comparar:

  * [ ] población
  * [ ] mortalidad
  * [ ] distribución de recursos
  * [ ] relaciones
  * [ ] hogares
  * [ ] migración
* [ ] Identificar puntos de divergencia
* [ ] Explicar diferencias causales

### Vertical de validación

Dos mundos idénticos reciben reglas distintas de distribución de alimentos. Varias generaciones después, Nexus muestra cómo y por qué divergieron.

---

# Phase 4 — Emergent Society

## Phase 4.1 — Settlements

* [ ] Campamentos
* [ ] Residencias
* [ ] Almacenes
* [ ] Construcción
* [ ] Caminos
* [ ] Crecimiento y abandono
* [ ] Migraciones
* [ ] Identidad del asentamiento

## Phase 4.2 — Production

* [ ] Trabajo
* [ ] Profesiones
* [ ] Herramientas
* [ ] Transformación de recursos
* [ ] Especialización
* [ ] Producción doméstica y colectiva
* [ ] Excedentes
* [ ] Escasez

## Phase 4.3 — Exchange

* [ ] Regalos
* [ ] Trueque
* [ ] Intercambio recurrente
* [ ] Deudas
* [ ] Reputación económica
* [ ] Propiedad individual
* [ ] Propiedad familiar
* [ ] Propiedad comunal
* [ ] Mercados
* [ ] Moneda emergente o institucional

## Phase 4.4 — Groups and Factions

* [ ] Grupos no familiares
* [ ] Identidad colectiva
* [ ] Objetivos compartidos
* [ ] Membresía
* [ ] Roles
* [ ] Liderazgo
* [ ] Confianza interna
* [ ] Rivalidad entre grupos
* [ ] Alianzas
* [ ] Fragmentación

## Phase 4.5 — Norms and Institutions

* [ ] Normas informales
* [ ] Reputación social
* [ ] Recompensas
* [ ] Castigos
* [ ] Autoridad
* [ ] Cumplimiento
* [ ] Resistencia
* [ ] Legitimidad
* [ ] Instituciones persistentes

## Phase 4.6 — Politics

* [ ] Liderazgo
* [ ] Sucesión
* [ ] Decisiones colectivas
* [ ] Distribución de poder
* [ ] Leyes
* [ ] Impuestos o contribuciones
* [ ] Conflictos políticos
* [ ] Rebeliones
* [ ] Diplomacia
* [ ] Guerra

---

# Phase 5 — Culture and Knowledge

## Phase 5.1 — Beliefs

* [ ] Creencias individuales
* [ ] Transmisión social
* [ ] Confianza en una creencia
* [ ] Mutación
* [ ] Contradicciones
* [ ] Conversión
* [ ] Pérdida de creencias

## Phase 5.2 — Culture

* [ ] Valores colectivos
* [ ] Costumbres
* [ ] Rituales
* [ ] Tabúes
* [ ] Celebraciones
* [ ] Nombres
* [ ] Símbolos
* [ ] Identidades culturales
* [ ] Mezcla y separación cultural

## Phase 5.3 — Information

* [ ] Rumores
* [ ] Distorsión de información
* [ ] Prestigio de las fuentes
* [ ] Propagación entre grupos
* [ ] Secretos
* [ ] Registros escritos
* [ ] Educación

## Phase 5.4 — Technology

* [ ] Conocimiento práctico
* [ ] Descubrimientos
* [ ] Transmisión
* [ ] Pérdida de conocimiento
* [ ] Especialistas
* [ ] Dependencias tecnológicas
* [ ] Difusión entre sociedades

---

# Phase 6 — History and Causal Analysis

## Phase 6.1 — Historical Memory

* [ ] Eventos históricos derivados
* [ ] Importancia del evento
* [ ] Memoria individual
* [ ] Memoria familiar
* [ ] Memoria colectiva
* [ ] Interpretaciones contradictorias

## Phase 6.2 — Timeline

* [ ] Línea temporal mundial
* [ ] Filtros por entidad, familia, grupo y lugar
* [ ] Navegación hacia el mapa
* [ ] Periodos históricos
* [ ] Comparación temporal

## Phase 6.3 — Legend Mode

* [ ] Biografía de una entidad
* [ ] Historia de un linaje
* [ ] Historia de un asentamiento
* [ ] Historia de una institución
* [ ] Árbol causal
* [ ] Pregunta “¿por qué ocurrió?”
* [ ] Pregunta “¿qué habría cambiado si...?”
* [ ] Exportación narrativa

## Phase 6.4 — Analytics

* [ ] Demografía
* [ ] Recursos
* [ ] Producción
* [ ] Desigualdad
* [ ] Movilidad social
* [ ] Relaciones entre grupos
* [ ] Migración
* [ ] Conflicto
* [ ] Difusión cultural
* [ ] Comparación entre escenarios

---

# Phase 7 — World Scale

* [ ] Simulación distante en menor resolución
* [ ] Niveles de detalle cognitivo
* [ ] Poblaciones agregadas
* [ ] Individuos históricos persistentes
* [ ] Materialización de población al acercarse
* [ ] Pathfinding jerárquico
* [ ] Flow fields para destinos concurridos
* [ ] Rutas compartidas
* [ ] Procesamiento multithread con Web Workers
* [ ] Sistemas espaciales en GPU cuando resulte apropiado
* [ ] Mundos con múltiples asentamientos
* [ ] Sociedades simultáneas

---

# Phase 8 — Product and Polish

* [ ] Tutorial
* [ ] Escenarios predefinidos
* [ ] Presets de reglas
* [ ] Editor de mundo
* [ ] Herramientas de observación
* [ ] Interfaz de comparación
* [ ] Accesibilidad
* [ ] Optimización de carga
* [ ] Tauri desktop
* [ ] Renderizador wgpu nativo
* [ ] Modding
* [ ] Formato compartible de escenarios

---

# Future Research

* [ ] Simulación ecológica profunda
* [ ] Clima cambiante
* [ ] Enfermedades
* [ ] Evolución genética
* [ ] Lenguajes emergentes
* [ ] Religiones
* [ ] Guerra territorial avanzada
* [ ] Comercio internacional
* [ ] Navegación marítima
* [ ] IA asistiendo en la creación de reglas
* [ ] Generación narrativa asistida
* [ ] Multiplayer para edición compartida

---

# Reglas técnicas del proyecto

Todo sistema nuevo debe definir:

1. **Estado:** qué información conserva.
2. **Comportamiento:** qué decisiones modifica.
3. **Eventos:** qué hechos produce.
4. **Causalidad:** cómo registra por qué sucedió.
5. **Observabilidad:** cómo puede inspeccionarlo el usuario.
6. **Frecuencia:** cuándo necesita actualizarse.
7. **Complejidad:** cómo escala con la población.
8. **Validación:** qué historia emergente demuestra que funciona.

Además:

* Ningún sistema debe recorrer toda la población cada tick sin una justificación medida.
* Las relaciones se almacenan únicamente entre individuos conocidos.
* Las reglas se indexan por los eventos que pueden activarlas.
* El pathfinding utiliza presupuestos y reutiliza resultados.
* Las decisiones se distribuyen entre ticks.
* La simulación debe conservar determinismo.
* Los benchmarks forman parte de la definición de terminado.
* Las optimizaciones deben conservar paridad de resultados.

## Prioridad inmediata

Aunque el roadmap sea enorme, el trabajo actual permanece muy acotado:

1. `Socialize`
2. `ApproachEntity`
3. `Interact`
4. Cambios deterministas de afinidad
5. Cooldown social
6. Relaciones influyendo en reproducción
7. Primer `SimulationEvent`
8. Historial de interacciones
