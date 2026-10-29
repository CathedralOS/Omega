rustc --edition 2024 -O build/proof-profile/snapshot-fusion/intersection_cases.rs -o build/proof-profile/snapshot-fusion/intersection_cases.exe
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& build/proof-profile/snapshot-fusion/intersection_cases.exe > build/proof-profile/snapshot-fusion/algorithm-result.json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& build/proof-profile/payload-index-build.ps1
exit $LASTEXITCODE
