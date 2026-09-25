$f = "src/main.rs"
$c = [System.IO.File]::ReadAllText($f)
$c =$c -replace 'fn new\(\) -> Self \{\s*users: sysinfo::Users::new_with_refreshed_list\(\),', 'fn new() -> Self {'
if ($c -notmatch 'pub users:\s*(sysinfo::)?Users') { 
    $c =$c -replace 'pub struct App \{', "pub struct App {`n    pub users: sysinfo::Users," 
}
if ($c -notmatch 'users:\s*(sysinfo::)?Users::new_with_refreshed_list\(\)') { 
    $c =$c -replace '(fn new\(\) -> Self \{\s*Self \{)', "`$1`n            users: sysinfo::Users::new_with_refreshed_list()," 
}
$c =$c -replace 'KeyCode::Char\(''k''\)\s*\|\s*KeyCode::Delete', 'KeyCode::Delete'
[System.IO.File]::WriteAllText($f,$c)
Write-Host "Patched src/main.rs successfully!" -ForegroundColor Green
