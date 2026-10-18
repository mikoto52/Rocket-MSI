# 설치·제거 실행 목록: 현재 구현 및 시험

Actions/Install/Action 및 Actions/Uninstall/Action을 문서 순서대로 동기 실행한다. 현재 빌더 GUI는 미구현이며 .embproj에서 설정한다.

```xml
<Actions>
  <Install>
    <Action id="configure" name="초기 설정" enabled="true" hideWindow="true"
            failurePolicy="notify" timeoutSeconds="60">
      <Executable kind="installedFile" path="bin/MyApp.exe" />
      <Arguments><Argument>--configure</Argument><Argument>{ProgramData}\MyApp</Argument></Arguments>
      <WorkingDirectory>{InstallDir}</WorkingDirectory>
    </Action>
    <Action id="optional" failurePolicy="ignore">
      <Executable kind="targetPath" path="{InstallDir}\Optional.exe" />
    </Action>
  </Install>
  <Uninstall>
    <Action id="cleanup" hideWindow="true" failurePolicy="stop">
      <Executable kind="installedFile" path="bin/MyApp.exe" />
      <Arguments><Argument>--cleanup</Argument></Arguments>
    </Action>
  </Uninstall>
</Actions>
```

## 오류 처리

| failurePolicy | 빌더 표시명 | 실패 시 동작 |
|---|---|---|
| ignore | 무시 | 로그에 남기고 다음 항목 실행 |
| notify | 알림 후 계속 | 전체/축소 UI에서는 알림 확인 후 계속. /passive·/qn은 창 없이 로그에 기록 |
| stop (기본) | 중단 | 다음 항목을 실행하지 않고 MSI 실패 및 MSI 관리 리소스 롤백 요청 |

파일 없음, 작업 폴더 없음, CreateProcess 시작 실패, 0 이외 종료 코드, 지정한 제한 시간 초과에 동일 정책을 적용한다. 3010 등도 현재 실패 코드로 취급한다. 사용자 취소는 정책과 무관하게 취소한다. 로그는 `InstallEmbly action Install/id` 또는 `Uninstall/id`로 시작하며 사유·정책·소요 시간을 기록한다.

installedFile은 프로젝트에 포함된 EXE의 상대 경로다. 잘못된 프로젝트 참조는 빌드 오류이며 ignore로 감추지 않는다. 설치 후 사용자가 파일을 삭제했거나, targetPath의 선택적 프로그램이 없는 경우는 실행 시 정책을 적용한다. targetPath는 대상 PC의 절대 EXE 경로 또는 기본 경로 변수로 시작하는 경로다. PATH 검색·파일 연결·암묵적인 셸 처리는 하지 않는다. 명령어가 필요하면 cmd.exe 등 실행기를 명시해야 하며 인수 해석은 그 프로그램의 규칙을 따른다. msiexec.exe 직접 지정은 중첩 설치 방지를 위해 차단한다.

name 생략 시 id를 사용한다. enabled 기본 true, hideWindow 기본 false, 인수 기본 빈 목록이다. WorkingDirectory 생략 시 실행 EXE의 부모 폴더다. 명시하면 절대 경로 템플릿을 사용한다. timeoutSeconds 생략 시 제한이 없고, 지정 시 1..86400이다. 목록당 최대 100개이며 id는 숫자로 시작하지 않는 영문·숫자·밑줄 최대 60자다. 각 목록 안에서 중복 id를 금지한다.

인수·대상 PC 경로·작업 폴더에는 현재 레지스트리와 같은 기본 변수(InstallDir, ProgramFiles, ProgramData, WindowsDir, ProductName, Manufacturer, ProductVersion)를 지원한다. 빈 인수·따옴표·공백·대괄호를 보존한다. NUL 및 내부 전송 구분자 U+001F는 금지한다. UTF-16 명령행·전송 데이터 길이도 제한한다.

## 실행 시점과 권한

- 신규 설치: 파일·바로가기·레지스트리 배치 이후, InstallFinalize 이전.
- 명시적 전체 제거: 파일·레지스트리 삭제 이전.
- 복구·동일 버전 재설치·롤백: 실행하지 않는다.
- 업그레이드는 아직 미구현이며 runOnUpgrade=true는 거부한다. 이전 제품 제거 시의 UPGRADINGPRODUCTCODE가 있으면 제거 목록을 건너뛴다.

Binary 테이블에 패키지 아키텍처별 Rust DLL을 포함하고 deferred/no-impersonate CustomAction으로 실행한다. 필요한 경로·인수는 CustomActionData로 전달한다. 설치·제거는 같은 오류 정책과 도우미를 사용한다. 창 숨김은 CREATE_NO_WINDOW와 SW_HIDE 요청이며 대상이 자체 생성하는 대화상자까지 숨기지는 못한다. 실행 컨텍스트는 상승된 설치 서비스이므로 창 표시를 켜도 로그인 사용자에게 보인다고 보장하지 않는다. 로그인 사용자가 앱을 여는 것은 별도의 완료 후 실행 기능을 사용한다.

취소·시간 초과 시 직접 시작한 프로세스만 종료한다. 자식 프로세스·서비스·외부 변경의 롤백은 보장하지 않는다. stop은 이미 실행한 외부 프로그램의 변경을 자동 복원하지 않는다. 실행 도우미는 인수·표준 출력을 로그로 출력하지 않으며 MSI 속성 숨김을 설정하지만 명령행에 전달한 비밀의 완전한 은닉은 보장하지 않는다.

## 빌드 및 자동 검증

빌더 빌드에는 x86/x64 Rust MSVC 타깃 및 대응하는 Visual C++ 빌드 도구가 필요하다.

```powershell
rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc
cargo test --offline
cargo clippy --offline -- -D warnings
rustc --edition=2024 --test src/action_runtime.rs -C target-feature=+crt-static -o target/action-runtime-tests.exe
& ./target/action-runtime-tests.exe
```

build.rs가 네이티브 DLL 두 개를 컴파일하여 빌더에 내장한다. CRT는 정적 링크하며 WiX나 실행 시 외부 Rust 크레이트는 필요하지 않다. 런타임 테스트의 ignored 세 항목은 부모 테스트가 자식 프로세스로 선택 실행하는 보조 항목이다. 자동 검증은 MSI 실제 설치를 수행하지 않는다.

## 시험 키트 생성

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\create-actions-demo.ps1"
```

dist/actions-demo에 main, stop-missing, stop-exit, stop-timeout의 독립된 MSI를 생성한다. 기본 예제는 examples/ActionsDemo.embproj다. 도우미 EXE는 지정된 시험 로그에만 행을 추가한다. 테스트 생성 스크립트는 MSI나 도우미를 실행하지 않는다.

## 1. 무시·알림 후 계속

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\actions-demo\main\Setup.msi" /L*V "D:\sources\Express-MSI-Builder\dist\actions-demo\main\install.log"
```

없는 대상 무시는 알림 없이 넘어가고, 종료 코드 17도 무시한다. 없는 대상 알림은 창을 표시하고 확인 후 마지막 항목까지 실행해야 한다. 설치가 끝난 후 다음 명령으로 순서·인수·작업 폴더를 확인한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-demo.ps1" -Phase Installed
```

probe 로그는 `%ProgramData%\InstallEmbly\ActionsDemo\main.log`이며 설치 순서는 install-first, install-exit17, install-last다. disabled는 MSI에 스케줄하지 않는다. 시험 로그는 MSI 관리 대상이 아니며 롤백·제거 이후에도 남긴다. 재시험 전에는 기존 로그를 별도 이름으로 보존해야 중복 행으로 인한 검사 실패를 피할 수 있다.

## 2. 복구 시 미실행 및 제거 전 실행

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-demo.ps1" -Phase BeforeRepair
```

같은 MSI에서 복구한 뒤 검사한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-demo.ps1" -Phase Repaired
```

로그가 변하지 않아야 한다. 제거 후 다음 명령으로 uninstall-first, uninstall-last가 뒤에 추가됐고 제거 당시 EXE가 존재했으며 제거 후 EXE는 없어졌는지 확인한다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-demo.ps1" -Phase Removed
```

일반 실행에서 이전에 관찰한 롤백 정리 권한 오류가 생기면 통과로 간주하지 않고 로그를 확인한다. 관리자 실행과 비교하고 부트스트래퍼는 후속 과제로 유지한다.

## 3. 중단과 롤백

무인 UI 검증은 아래 별도 절차를 따른다.

각 stop-* 폴더의 Setup.msi를 /i 및 /L*V로 실행한다. 대상 없음, 종료 코드 17, 1초 제한 초과로 각각 실패해야 한다. install-last가 실행되지 않고 설치 EXE가 롤백되어야 한다. 예:

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\actions-demo\stop-missing\Setup.msi" /L*V "D:\sources\Express-MSI-Builder\dist\actions-demo\stop-missing\install.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-demo.ps1" -Phase Stopped -Variant stop-missing
```

MSI가 오류로 종료된 뒤 검사 명령을 실행한다. stop-exit, stop-timeout도 동일하게 바꿔 시험한다. 설치 경로를 변경한 경우 검사에 -InstallDir을 지정한다. 제거 중 stop·알림, /passive·/qn의 창 억제, 실제 콘솔/GUI 창 및 취소의 추가 수동 시험도 필요하다. 자동 테스트 결과만으로 해당 시험을 통과 처리하지 않는다.

## 4. /passive 및 /qn 검증

기본 main 제품을 제거한 상태에서 시작한다. test-actions-unattended.ps1은 기존 probe 로그를 변경하지 않고 기준 스냅샷 이후에 추가된 기록만 검사한다. 각 모드에서 설치 → 복구 → 제거를 마친 후 다음 모드로 넘어간다. 아래는 passive 절차다.

```powershell
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-unattended.ps1" -Mode passive -Phase Prepare
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\actions-demo\main\Setup.msi" /passive /L*V "D:\sources\Express-MSI-Builder\dist\actions-demo\main\passive-install.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-unattended.ps1" -Mode passive -Phase Installed
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\actions-demo\main\Setup.msi" REINSTALL=ALL REINSTALLMODE=omus /passive /L*V "D:\sources\Express-MSI-Builder\dist\actions-demo\main\passive-repair.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-unattended.ps1" -Mode passive -Phase Repaired
msiexec.exe /x "D:\sources\Express-MSI-Builder\dist\actions-demo\main\Setup.msi" /passive /L*V "D:\sources\Express-MSI-Builder\dist\actions-demo\main\passive-remove.log"
powershell -NoProfile -File "D:\sources\Express-MSI-Builder\scripts\test-actions-unattended.ps1" -Mode passive -Phase Removed
```

각 MSI 작업이 끝난 뒤 검사 명령을 실행한다. quiet 시험은 위 명령의 -Mode passive를 -Mode quiet로, /passive를 /qn으로, 로그 이름의 passive-를 quiet-로 바꾼다. 관리자 PowerShell에서 실행하며 quiet는 Start-Process -Wait로 MSI 종료를 기다리거나 프로세스가 끝난 뒤 검사한다.

passive는 진행 화면만, quiet는 MSI 화면 없이 동작해야 한다. 두 모드 모두 notify는 로그만 남기고 확인을 요구하는 알림이 나타나지 않는지 사용자가 확인한다. UAC 승인은 MSI의 UI 수준과 별개다. 검사 스크립트는 서버 UILevel=3/2, 성공 코드, 실행 순서·오류 정책, 복구 시 미실행, 제거를 확인한다. 기준 파일은 덮어쓰지 않으며 재시험 시 별도 이름으로 보존한 뒤 준비한다.

참고: [Deferred CustomAction](https://learn.microsoft.com/en-us/windows/win32/msi/deferred-execution-custom-actions), [보안과 속성 전달](https://learn.microsoft.com/en-us/windows/win32/msi/custom-action-security), [MSI 오류 다이얼로그](https://learn.microsoft.com/en-us/windows/win32/msi/error-dialog).
