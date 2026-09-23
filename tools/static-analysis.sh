#!/usr/bin/env bash
# Run the first-pass static analysis for project-owned C sources.
#
# GCC's -fanalyzer is enabled through CMake so that diagnostics use the same
# include paths and definitions as the normal build. The API audit below is
# intentionally small and high-signal; it catches unsafe legacy interfaces
# before a broader checker is introduced.
set -euo pipefail

readonly SCRIPT_NAME="$(basename "$0")"
readonly SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly DEFAULT_BUILD_DIR="${SOURCE_DIR}/build/static-analysis"
readonly PROJECT_C_PATH="src/cargopit"
readonly UNSAFE_API_PATTERN='(^|[^[:alnum:]_])(gets|strcpy|strcat|sprintf|vsprintf|scanf|sscanf|fscanf|strncpy|strncat|atoi|atol|atof|system|popen)[[:space:]]*\('
readonly CI_WARNING_PATTERN='warning:.*\[-W(analyzer-|array-bounds|stringop-overflow|format-overflow|format-truncation|return-type|free-nonheap-object|use-after-free)'
readonly CI_WARNING_EXCLUDE_PATTERN="/src/cargopit/simulatorapi/"

build_dir="${DEFAULT_BUILD_DIR}"
strict=0
skip_build=0
ci=0

usage() {
    cat <<EOF
Usage: ${SCRIPT_NAME} [options]

Configure and build the project-owned C targets with GCC static analysis,
then report commonly unsafe C APIs in the legacy source tree.

Options:
  --build-dir DIR  Use DIR for the analysis build
  --skip-build     Only run the source API audit
  --strict         Return failure when the API audit finds a match and treat
                   compiler diagnostics as errors
  --ci             Fail on high-confidence analyzer diagnostics and unsafe
                   API findings; intended for continuous integration
  -h, --help       Show this help
EOF
}

fail() {
    printf '%s: %s\n' "${SCRIPT_NAME}" "$1" >&2
    exit 2
}

while (($# > 0)); do
    case "$1" in
        --build-dir)
            (($# >= 2)) || fail "--build-dir requires a directory"
            build_dir="$2"
            shift 2
            ;;
        --skip-build)
            skip_build=1
            shift
            ;;
        --strict)
            strict=1
            shift
            ;;
        --ci)
            ci=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1"
            ;;
    esac
done

if ((ci == 1 && skip_build == 1)); then
    fail "--ci requires a build; remove --skip-build"
fi

if [[ "${build_dir}" != /* ]]; then
    build_dir="${SOURCE_DIR}/${build_dir}"
fi

if ((skip_build == 0)); then
    cmake_command="${CMAKE:-cmake}"
    c_compiler="${CC:-gcc}"
    cxx_compiler="${CXX:-g++}"

    command -v "${cmake_command}" >/dev/null 2>&1 ||
        fail "cmake is required"
    command -v "${c_compiler}" >/dev/null 2>&1 ||
        fail "C compiler '${c_compiler}' is required"
    command -v "${cxx_compiler}" >/dev/null 2>&1 ||
        fail "C++ compiler '${cxx_compiler}' is required by CMake"

    cmake_args=(
        -S "${SOURCE_DIR}"
        -B "${build_dir}"
        -DBUILD_TUI=OFF
        -DENABLE_TESTS=ON
        -DENABLE_STATIC_ANALYSIS=ON
        -DSTATIC_ANALYSIS_AS_ERRORS=$([[ "${strict}" -eq 1 ]] && printf ON || printf OFF)
        -DCMAKE_BUILD_TYPE=Debug
        -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
        "-DCMAKE_C_COMPILER=${c_compiler}"
        "-DCMAKE_CXX_COMPILER=${cxx_compiler}"
    )

    "${cmake_command}" "${cmake_args[@]}"
    analysis_log="${build_dir}/static-analysis.log"
    if ! "${cmake_command}" --build "${build_dir}" --parallel 2>&1 |
        tee "${analysis_log}"; then
        exit 1
    fi

    if ((ci == 1)); then
        ci_findings="$(
            grep -nE "${CI_WARNING_PATTERN}" "${analysis_log}" |
                grep -vF "${CI_WARNING_EXCLUDE_PATTERN}" ||
                true
        )"
        if [[ -n "${ci_findings}" ]]; then
            printf 'High-confidence static analysis diagnostics found:\n%s\n' \
                "${ci_findings}" >&2
            exit 1
        fi
        printf 'CI static analysis diagnostic gate passed.\n'
    fi
fi

git_command="${GIT:-git}"
command -v "${git_command}" >/dev/null 2>&1 ||
    fail "git is required for the source API audit"

api_findings="$(
    "${git_command}" -C "${SOURCE_DIR}" grep -nE \
        -e "${UNSAFE_API_PATTERN}" -- \
        "${PROJECT_C_PATH}" \
        ":(exclude)${PROJECT_C_PATH}/simulatorapi/**" ||
        true
)"

if [[ -n "${api_findings}" ]]; then
    printf 'Common unsafe C APIs found in project-owned sources:\n'
    while IFS= read -r finding; do
        printf '  %s\n' "${finding}"
    done <<< "${api_findings}"
    if ((strict == 1 || ci == 1)); then
        exit 1
    fi
else
    printf 'Common unsafe C API audit passed.\n'
fi

printf 'Static analysis completed for project-owned C targets.\n'
