# CAB 출력 및 분할 설계

최신 사용자 검증(2026-10-18): 단일 외부 CAB 시험본에 대해 **통과** 보고를 받았다. 이는 아래 구현 당시의 시험 대기 상태를 갱신한다. CAB 누락·손상 등 개별 실패 시나리오의 수동 시험 결과는 별도 보고 전까지 완료로 간주하지 않는다.

2026-10-18 사용자 요구. 현재 CLI는 **내장/외부 CAB 저장 및 파일 단위 크기 분할**을 지원한다. Tauri 빌드 화면과 단일 파일의 여러 CAB 걸침(spanning)은 후속 개발 계획이다. 최신 동작과 시험은 [분할 CAB 시험](split-cab-test.md)을 따른다.

## 빌드 화면

출력 폴더와 함께 CAB 저장 방식을 선택한다.

- **MSI에 CAB 포함**: 기본값. MSI 내부 스트림에 CAB 저장, 배포물은 MSI 하나.
- **CAB 따로 두기**: MSI와 같은 출력 폴더에 CAB 저장, 배포물은 MSI 및 모든 CAB.

분할 설정은 저장 방식과 독립적으로 설계한다. 분할 안 함/크기 기준 자동 분할, CAB당 목표 크기(MiB)를 제공할 계획이다. 값의 기본값·최솟값·최댓값은 압축 및 MSI 제약 검증 후 확정한다. 외부 CAB의 이름은 Data1.cab, Data2.cab 순으로 생성한다. 실제 압축 결과와 형식 한계에 따라 크기 목표를 지키도록 재패키징하거나 빌드를 명확히 실패시킨다.

화면에는 예상 배포 파일 목록을 표시하고 빌드 완료 시 실제 MSI/CAB 전체 목록 및 크기를 표시한다. 외부 CAB 방식에서는 MSI만 복사하면 설치할 수 없음을 배포물 안내에 표시한다.

## 엔진 설계

압축 계획 모델에 CAB의 파일 목록, 이름, 파일 순서 범위, 저장 방식을 둔다. 파일 수집 → 분할 계획 → CAB 생성 → 실제 결과 검증 → Media 테이블 및 스트림 생성 순으로 처리한다.

Media.Cabinet은 내장 CAB에 `#Data1.cab`, 외부 CAB에 `Data1.cab`을 기록한다. 내장은 대응하는 이름의 _Streams 항목에 저장하고 외부 파일은 소스 루트에 배치한다. 각 CAB의 내부 파일 식별자·순서를 File 테이블과 맞추며 Media의 DiskId/LastSequence를 실제 결과에 맞춰 작성한다. 단일 파일이 CAB 경계를 넘는 spanning은 별도 시험 없이 지원한다고 가정하지 않는다.

외부 결과물은 작업 폴더에서 모두 생성·검증한 뒤 하나의 결과 세트로 공개한다. 기존 파일 충돌, 생성 실패·취소 시 새 MSI만 남는 부분 배포를 방지한다. CAB를 임의 이름으로 바꾸거나 다른 빌드의 CAB를 섞지 않도록 결과 목록을 제공한다. 외부 CAB의 디지털 서명/검증 통합은 별도 후속 설계 대상으로 기록한다.

## XML 초안

현재 예제는 단일 CAB 내장 기본값으로 유지하고, 후속 스키마에 다음과 같은 선택적 요소를 검토한다.

```xml
<Build outputDirectory="./dist">
  <Cabinet storage="external" split="size" targetSizeMiB="512" />
</Build>
```

위 512는 설명용 값이며 지원 한도나 확정 기본값이 아니다. storage는 embedded/external, split은 none/size를 구분한다. 기존처럼 Cabinet 요소가 없으면 단일 내장 CAB로 해석하는 호환 정책을 제안한다.

## 단계와 검증

1. 단일 CAB 내장: 현재 구현.
2. 단일 외부 CAB: CLI 선택 및 배포 세트 생성 구현. 실제 설치·복구·제거 시험 대기.
3. 여러 파일 분할: 작은 시험 한도로 Data1.cab/Data2.cab 경계와 파일 누락·중복 확인.
4. 대용량: 총 입력 4GiB 이상, 단일 파일 크기 경계, 압축이 잘 안 되는 입력, CAB/MSI 형식 한도 및 디스크 부족·취소 시험.

분할 CAB가 있다고 단일 초대형 파일이나 MSI 자체의 크기 한도가 자동으로 해결되는 것은 아니다. 미지원 입력은 빌드 전에 진단한다. 검증된 한도만 사용자에게 안내하고 4GiB 이상 지원은 실제 시험 후 선언한다. 설치·복구 시 외부 CAB 누락/손상과 원본 소스 이동도 시험한다.

공식 근거: [Media Table](https://learn.microsoft.com/en-us/windows/win32/msi/media-table), [Cabinet 데이터 형식](https://learn.microsoft.com/en-us/windows/win32/msi/cabinet), [Cabinet Files](https://learn.microsoft.com/en-us/windows/win32/msi/cabinet-files).

## 단일 외부 CAB 구현 및 시험

```powershell
cargo run --offline -- build examples/payload/hello.txt dist/external-demo/Setup.msi --cab-storage external --license examples/demo-terms.txt
cargo run --offline -- build examples/payload/hello.txt dist/embedded-demo/Setup.msi --cab-storage embedded --license examples/demo-terms.txt
```

`--cab-storage` 생략 시 embedded다. 현재 내장 CAB의 기존 스트림명 payload.cab은 유지한다. 외부 모드는 Media.Cabinet=Data1.cab이며 CAB 스트림을 MSI에 넣지 않는다. 출력 MSI마다 별도 폴더 사용을 권장하며, 기존 MSI나 Data1.cab이 있으면 덮어쓰지 않고 실패한다.

작업 폴더의 MSI/CAB를 먼저 검증하고 CAB를 공개한 다음 MSI를 마지막에 공개한다. MSI 공개 실패 시 이번에 만든 CAB를 삭제한다. 프로세스 강제 종료·전원 장애까지 파일 두 개의 원자적 공개를 보장하지는 않는다. 작업 중 실패/충돌은 기존 파일을 보존한다.

verify 명령은 외부 CAB의 존재, 헤더와 기록된 크기를 검사한다. 이는 전체 압축 무결성·서명 검증을 의미하지 않는다. 자동 테스트는 실제 CAB를 expand.exe로 해제하여 입력 내용과 대조하고, CAB 누락·손상 헤더·출력 충돌 및 공개 실패 정리를 확인한다. 기존 내장/약관/유지보수 회귀 검사와 정적 검사도 통과했다.

시험 결과물:

- dist/external-demo/Setup.msi 및 Data1.cab
- dist/embedded-demo/Setup.msi

외부 모드 시험 환경에서는 **Setup.msi와 Data1.cab을 같은 폴더에 함께 복사**한다. 이전 시험 제품을 제거하거나 깨끗한 VM에서 시작한다. 모두 같은 시제품 ProductCode이므로 기존 패키지 캐시를 섞어 시험하지 않는다.

```powershell
msiexec.exe /i "D:\sources\Express-MSI-Builder\dist\external-demo\Setup.msi" /L*v "D:\sources\Express-MSI-Builder\dist\external-demo\install.log"
```

확인: 신규 설치 → 파일 삭제 후 복구 → 제거. 별도 시험에서는 CAB를 다른 폴더로 이동했을 때 실패/원본 요청을 확인하고, CAB를 복원해 다시 설치·복구한다. 실패 시 제품/파일이 부분 설치 상태로 남는지도 확인한다. 개발 PC에서 실제 설치를 자동 수행하지 않았으므로 설치·복구·제거 결과는 아직 미검증이다.

후속 갱신(2026-10-18): 폴더 입력으로 여러 파일·하위 경로를 유지하는 기능을 구현했다. 단일 파일 입력의 hello.txt 동작은 유지한다. [다중 파일 시험](multi-file-test.md)을 참고한다. CAB 분할과 대용량 지원은 아직 없다.
