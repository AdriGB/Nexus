Set-StrictMode -Version Latest

$script:BenchmarkResultsSchemaVersion = 1

function Write-BenchmarkResults {
    param(
        [Parameter(Mandatory = $true)][string]$Suite,
        [Parameter(Mandatory = $true)][string[]]$ScenarioNames,
        [Parameter(Mandatory = $true)][string]$OutputDir
    )

    $uniqueNames = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    $orderedNames = [System.Collections.Generic.List[string]]::new()
    foreach ($name in $ScenarioNames) {
        if (-not $uniqueNames.Add($name)) {
            throw "Duplicate scenario '$name' cannot be aggregated."
        }
        $orderedNames.Add($name)
    }
    $orderedNames.Sort([System.StringComparer]::Ordinal)

    $results = @()
    foreach ($name in $orderedNames) {
        $path = Join-Path $OutputDir "$name.json"
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Scenario result '$path' does not exist."
        }

        try {
            $payload = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        }
        catch {
            throw "Scenario result '$path' contains invalid JSON: $($_.Exception.Message)"
        }

        if ($payload.PSObject.Properties.Name -notcontains "schema_version") {
            throw "Scenario result '$path' has no schema_version."
        }
        if ($payload.PSObject.Properties.Name -notcontains "scenario" -or
            $null -eq $payload.scenario -or
            $payload.scenario.PSObject.Properties.Name -notcontains "name") {
            throw "Scenario result '$path' has no scenario.name."
        }
        if ($payload.scenario.name -cne $name) {
            throw "Scenario result name '$($payload.scenario.name)' does not match '$name'."
        }

        $results += $payload
    }

    $aggregate = [ordered]@{
        schema_version = $script:BenchmarkResultsSchemaVersion
        suite = $Suite
        results = $results
    }
    $json = $aggregate | ConvertTo-Json -Depth 100
    $aggregatePath = Join-Path $OutputDir "benchmark-results.json"
    [System.IO.File]::WriteAllText(
        $aggregatePath,
        $json + [Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    return $aggregatePath
}
