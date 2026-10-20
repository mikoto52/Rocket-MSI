# ARM64 패키지 생성

0.3.0부터 GUI **제품 정보 → 아키텍처 → ARM64** 또는 `.embproj`의 `Product architecture="arm64"`로 ARM64 MSI를 생성한다. 빌더는 기존 x64 실행 파일에서 ARM64 패키지를 생성하며, 빌더 자체의 ARM64 실행 파일 배포는 이번 범위에 포함하지 않는다.

- Summary Information의 Template은 `Arm64;1042`, 최소 Windows Installer 버전은 `500`이다.
- 파일·레지스트리·유지보수 컴포넌트는 64비트 속성을 사용한다. ProgramFiles64Folder, System64Folder 및 레지스트리 자동 영역은 ARM64의 네이티브 64비트 경로로 연결한다.
- 실행 도우미와 Quick MSI Embedded UI는 `aarch64-pc-windows-msvc`로 별도 컴파일한 ARM64 DLL을 포함한다.
- 아키텍처별 컴포넌트 GUID는 서로 구분한다. ARM64를 선택해도 입력 EXE/DLL을 ARM64로 변환하거나 개별 입력 파일의 CPU 형식을 강제 검사하지는 않는다.

원본 파일은 ARM64용으로 별도 빌드해 준비한다. x64·ARM64 제품 간 업그레이드 정책은 배포자가 UpgradeCode와 설치 위치를 고려해 정한다.

## 테스트 샘플

```powershell
.\dist\builder\RocketMsiBuilderCli.exe build .\examples\Arm64Demo.embproj
```

샘플은 텍스트 파일과 ARM64 실행 도우미를 검사하기 위한 `cmd.exe /c "exit /b 0"` 실행 항목을 포함하며 설치는 자동 실행하지 않는다. 설치·복구·제거와 Quick MSI 표시는 실제 Windows ARM64에서 수동 확인한다.

개발 PC에서는 x86/x64/ARM64의 실제 MSI 테이블, 내장 Quick MSI DLL의 PE CPU 형식·내보내기, 파일·레지스트리·바로가기·실행 시점의 회귀 테스트를 수행한다. x64 PC에서 ARM64 패키지를 생성한 결과만으로 ARM64 장치에서의 실행 검증이 완료된 것으로 간주하지 않는다.

[Microsoft 64-bit Windows Installer 패키지 요구사항](https://learn.microsoft.com/en-us/windows/win32/msi/64-bit-windows-installer-packages)
