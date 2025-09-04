# Module manifest for OpenAgent Terminal PowerShell integration

@{
    # Script module or binary module file associated with this manifest.
    RootModule = 'openagent_integration.ps1'

    # Version number of this module.
    ModuleVersion = '1.0.0'

    # Supported PSEditions
    CompatiblePSEditions = @('Desktop', 'Core')

    # ID used to uniquely identify this module
    GUID = 'a3b8c5d0-1e4f-4a9b-8c7d-2f5e8a1b3c6d'

    # Author of this module
    Author = 'OpenAgent Terminal Contributors'

    # Company or vendor of this module
    CompanyName = 'OpenAgent Terminal'

    # Copyright statement for this module
    Copyright = '(c) OpenAgent Terminal Contributors. Licensed under Apache 2.0.'

    # Description of the functionality provided by this module
    Description = 'PowerShell integration for OpenAgent Terminal providing OSC 133 command block tracking and terminal enhancements.'

    # Minimum version of the PowerShell engine required by this module
    PowerShellVersion = '5.1'

    # Modules that must be imported into the global environment prior to importing this module
    RequiredModules = @()

    # Assemblies that must be loaded prior to importing this module
    RequiredAssemblies = @()

    # Script files (.ps1) that are run in the caller's environment prior to importing this module.
    ScriptsToProcess = @()

    # Type files (.ps1xml) to be loaded when importing this module
    TypesToProcess = @()

    # Format files (.ps1xml) to be loaded when importing this module
    FormatsToProcess = @()

    # Functions to export from this module, for best performance, do not use wildcards and do not delete the entry, use an empty array if there are no functions to export.
    FunctionsToExport = @(
        'Test-OpenAgentOSC133',
        'Disable-OpenAgentOSC133', 
        'Show-OpenAgentIntegrationStatus'
    )

    # Cmdlets to export from this module, for best performance, do not use wildcards and do not delete the entry, use an empty array if there are no cmdlets to export.
    CmdletsToExport = @()

    # Variables to export from this module
    VariablesToExport = @()

    # Aliases to export from this module, for best performance, do not use wildcards and do not delete the entry, use an empty array if there are no aliases to export.
    AliasesToExport = @(
        'openagent-test',
        'openagent-disable',
        'openagent-status'
    )

    # List of all files packaged with this module
    FileList = @(
        'openagent_integration.ps1',
        'OpenAgent.psd1',
        'README.md'
    )

    # Private data to pass to the module specified in RootModule/ModuleToProcess. This may also contain a PSData hashtable with additional module metadata used by PowerShell.
    PrivateData = @{
        PSData = @{
            # Tags applied to this module. These help with module discovery in online galleries.
            Tags = @('Terminal', 'OpenAgent', 'OSC133', 'CommandBlocks', 'Shell', 'Integration')

            # A URL to the license for this module.
            LicenseUri = 'https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE'

            # A URL to the main website for this project.
            ProjectUri = 'https://github.com/GeneticxCln/OpenAgent-Terminal'

            # ReleaseNotes of this module
            ReleaseNotes = 'Initial release of PowerShell integration for OpenAgent Terminal'

            # Prerelease string of this module
            # Prerelease = ''

            # Flag to indicate whether the module requires explicit user acceptance for install/update/save
            RequireLicenseAcceptance = $false

            # External dependent modules of this module
            ExternalModuleDependencies = @()
        }
    }

    # HelpInfo URI of this module
    HelpInfoURI = 'https://docs.openagent-terminal.org/shell-integration/powershell'
}
