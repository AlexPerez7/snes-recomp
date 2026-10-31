# Bibliotecas nativas de la prueba de factibilidad (android-feasibility-spike):
#   libsnesrt.so  el runtime en modo biblioteca (SNES_RT_LIBRARY), con clang -O2: nada de la ROM
#   libspike.so   JNI: el generador (Rust, estático), libtcc (TinyCC compilado aquí) y la medición
cmake_minimum_required(VERSION 3.22.1)
project(snesspike C)

foreach(v SNES_RECOMP_DIR TINYCC_DIR SNES_GEN_LIB)
    if(NOT DEFINED ${v})
        message(FATAL_ERROR "falta ${v} (lo pasa app/build.gradle)")
    endif()
endforeach()
if(NOT EXISTS ${TINYCC_DIR}/libtcc.c)
    message(FATAL_ERROR "no están las fuentes de TinyCC en ${TINYCC_DIR} (ver spikes/android/README.md)")
endif()
if(EXISTS ${TINYCC_DIR}/config.h)
    message(FATAL_ERROR "${TINYCC_DIR}/config.h es de ./configure para el PC: usa una copia de TinyCC sin configurar")
endif()
if(NOT EXISTS ${SNES_GEN_LIB})
    message(FATAL_ERROR "no está ${SNES_GEN_LIB}: la tarea cargoGenerator de Gradle lo construye (rustup target add aarch64-linux-android)")
endif()

# Medimos el código optimizado también en la variante debug (la que se instala sin firma de release).
set(OPT -O2)

add_library(snesrt SHARED ${SNES_RECOMP_DIR}/runtime/headless.c)
target_compile_definitions(snesrt PRIVATE SNES_RT_LIBRARY SNES_RT_TICK)
target_compile_options(snesrt PRIVATE ${OPT} -std=c11 -w)
target_link_libraries(snesrt m)

add_library(tcc STATIC ${TINYCC_DIR}/libtcc.c)
target_include_directories(tcc PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/tcc_config ${TINYCC_DIR})
target_compile_definitions(tcc PRIVATE ONE_SOURCE=1 TCC_TARGET_ARM64=1 TARGETOS_ANDROID=1 _GNU_SOURCE)
target_compile_options(tcc PRIVATE ${OPT} -w -fPIC)

add_library(snesgen STATIC IMPORTED)
set_target_properties(snesgen PROPERTIES IMPORTED_LOCATION ${SNES_GEN_LIB})

add_library(spike SHARED spike_jni.c)
target_include_directories(spike PRIVATE ${TINYCC_DIR})
target_compile_options(spike PRIVATE ${OPT} -Wall)
target_link_libraries(spike snesrt tcc snesgen log dl m)
