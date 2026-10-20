# MSI 전자서명

Rocket MSI 0.3.0부터 빌드한 MSI에 SHA-256 Authenticode 서명을 적용한다. Windows SDK의 SignTool을 사용하며 PATH 또는 설치된 Windows Kits에서 자동 탐색한다. 별도 위치는 `ROCKET_MSI_SIGNTOOL` 환경 변수로 지정한다.

## GUI

**빌드 → MSI 전자서명 → MSI에 전자서명 적용**을 선택한다.

- **PFX 파일**: PFX/P12 파일을 선택한다. 빌드할 때 암호 입력 창이 나타나며 빈 암호도 입력할 수 있다. 암호는 프로젝트·빌드 로그에 저장하지 않는다.
- **Windows 인증서 저장소**: 현재 사용자 또는 로컬 컴퓨터의 개인(My) 저장소를 선택하고 **인증서 선택…**을 누른다. 유효기간 내에 있고 개인 키가 있으며 코드 서명 용도로 사용할 수 있는 인증서를 표시한다. 로컬 컴퓨터 키의 사용 권한은 Windows 설정을 따른다.
- **타임스탬프 서버**: RFC 3161 HTTP(S) 주소를 입력하면 SHA-256 타임스탬프를 적용한다. 비워두면 타임스탬프 없이 서명한다.

인증서를 자동 발급하거나 구매하지 않으며 PFX를 인증서 저장소에 자동 등록하지 않는다. PFX 서명 시 암호는 SignTool의 `/p` 인수로 전달되므로 실행 중인 프로세스의 명령행을 읽을 수 있는 계정에는 노출될 수 있다. 프로세스 인수에 암호를 전달하지 않으려면 인증서 저장소 방식을 사용한다.

## 프로젝트 설정과 CLI

```xml
<Build outputDirectory="dist" outputFile="Setup.msi">
  <Cabinet storage="embedded" split="none" />
  <Signing enabled="true" source="pfx" pfxFile="certs/publisher.pfx"
           timestampUrl="http://timestamp.digicert.com" />
</Build>
```

PFX 경로는 `.embproj` 폴더 기준 상대 경로 또는 절대 경로다. 인증서 저장소는 다음과 같이 설정한다. 지문은 인증서를 찾는 SHA-1 식별자이며 MSI 서명 알고리즘은 SHA-256이다.

```xml
<Signing enabled="true" source="store" storeLocation="currentUser"
         thumbprint="0123456789ABCDEF0123456789ABCDEF01234567"
         timestampUrl="http://timestamp.digicert.com" />
```

`storeLocation`은 `currentUser` 또는 `localMachine`이다. `Signing` 생략 또는 `enabled="false"`는 서명하지 않는다. `password` 등 암호 속성은 지원하지 않으며 XML 검증에서 거부한다. 비활성 서명 설정은 정규화·GUI 저장 시 생략한다.

CLI에서 PFX 암호는 빌드 프로세스의 `ROCKET_MSI_PFX_PASSWORD` 환경 변수로 전달한다. CI에서는 비밀 변수로 설정하고 명령행이나 로그에 암호를 기록하지 않는다. 빈 암호의 PFX는 환경 변수가 없어도 사용할 수 있다.

```powershell
.\RocketMsiBuilderCli.exe build .\Product.embproj
.\RocketMsiBuilderCli.exe verify-signature .\dist\Setup.msi
```

## 결과와 검증 범위

임시 MSI를 생성한 뒤 서명과 `signtool verify /pa /all`을 실행한다. 타임스탬프를 요청했다면 검증에 `/tw`도 사용한다. 서명·타임스탬프·신뢰 체인 검증 실패 시 빌드가 실패하며 최종 MSI와 외부 CAB는 게시하지 않는다. 기존 출력 파일은 덮어쓰지 않는다. 외부 CAB 자체에 서명을 붙이거나 원본 EXE/DLL을 서명하는 기능은 포함하지 않는다.

`scripts/test-signing.ps1`은 임시 자체 서명 PFX와 CurrentUser/My의 임시 인증서로 실제 MSI 서명을 검사한다. 신뢰되지 않은 인증서의 검증 실패 및 출력 차단도 검사한다. 신뢰 루트 저장소는 변경하지 않으며 테스트 후 인증서·개인 키·PFX를 제거한다. `-TimestampUrl http://timestamp.digicert.com` 옵션으로 실제 RFC 3161 타임스탬프도 검사했다. 실제 발급 인증서의 신뢰 체인에 대한 최종 배포 검증은 별도로 수행한다.

[Microsoft SignTool 문서](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool)
