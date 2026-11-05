# Bibliotecas nativas de la app Android de snes-port:
#   libSDL2.so  SDL2 (fuentes hermanas, ver gradle.properties)
#   libmain.so  el frontal de runtime/play.c en modo biblioteca (SNES_RT_LIBRARY, clang -O2), el generador (Rust,
#               estático), libtcc (TinyCC) y launcher.c, que genera y compila el juego al arrancar y llama al frontal
cmake_minimum_required(VERSION 3.22.1)
project(snesport C)

foreach(v SNES_RECOMP_DIR TINYCC_DIR SDL2_DIR SNES_GEN_LIB)
    if(NOT DEFINED ${v})
        message(FATAL_ERROR "falta ${v} (lo pasa app/build.gradle)")
    endif()
endforeach()
if(NOT EXISTS ${TINYCC_DIR}/libtcc.c)
    message(FATAL_ERROR "no están las fuentes de TinyCC en ${TINYCC_DIR} (ver android/gradle.properties)")
endif()
if(EXISTS ${TINYCC_DIR}/config.h)
    message(FATAL_ERROR "${TINYCC_DIR}/config.h es de ./configure para el PC: usa una copia de TinyCC sin configurar")
endif()
if(NOT EXISTS ${SDL2_DIR}/CMakeLists.txt)
    message(FATAL_ERROR "no están las fuentes de SDL2 en ${SDL2_DIR} (ver android/gradle.properties)")
endif()
if(NOT EXISTS ${SNES_GEN_LIB})
    message(FATAL_ERROR "no está ${SNES_GEN_LIB}: la tarea cargoGenerator de Gradle lo construye (rustup target add aarch64-linux-android)")
endif()

# 16 KB de alineación de páginas (Android 15+ y Google Play): el NDK 27 aún no lo hace por defecto.
add_link_options(-Wl,-z,max-page-size=16384)

set(SDL_SHARED ON CACHE BOOL "" FORCE)
set(SDL_STATIC OFF CACHE BOOL "" FORCE)
set(SDL_TEST OFF CACHE BOOL "" FORCE)
add_subdirectory(${SDL2_DIR} SDL2-build)

add_library(tcc STATIC ${TINYCC_DIR}/libtcc.c)
target_include_directories(tcc PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/tcc_config ${TINYCC_DIR})
target_compile_definitions(tcc PRIVATE ONE_SOURCE=1 TCC_TARGET_ARM64=1 TARGETOS_ANDROID=1 _GNU_SOURCE)
target_compile_options(tcc PRIVATE -O2 -w -fPIC)

add_library(snesgen STATIC IMPORTED)
set_target_properties(snesgen PROPERTIES IMPORTED_LOCATION ${SNES_GEN_LIB})

add_library(main SHARED launcher.c ${SNES_RECOMP_DIR}/runtime/play.c)
target_include_directories(main PRIVATE ${TINYCC_DIR} ${SNES_RECOMP_DIR}/runtime)
target_compile_definitions(main PRIVATE SNES_RT_LIBRARY SNES_RT_TICK)
target_compile_options(main PRIVATE -O2 -std=gnu11 -w -D_GNU_SOURCE)
target_link_libraries(main SDL2 tcc snesgen log dl m android)
