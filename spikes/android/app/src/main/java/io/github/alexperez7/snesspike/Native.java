package io.github.alexperez7.snesspike;

/** Lo nativo de la prueba (app/src/main/cpp/spike_jni.c). */
public final class Native {
    static {
        System.loadLibrary("spike");
    }

    private Native() {}

    /** Genera el C del juego en {@code outDir}; devuelve "OK: ..." con el tiempo o "ERROR: ...". */
    public static native String generate(byte[] rom, String profileToml, String[] gameNames, String[] gameTexts, String outDir);

    /**
     * Compila con libtcc el C de {@code srcDir} y ejecuta {@code frames} fotogramas sin vídeo, midiendo. mode 0 en memoria,
     * 1 como .so cargado con dlopen (plan B). cpu &gt;= 0 fija el núcleo. Si la ejecución empieza no vuelve (el proceso
     * termina y deja {@code outDir}/result.txt); si vuelve, es con el error.
     */
    public static native String bench(String srcDir, String tccInclude, String libDir, String romPath, int frames, int cpu, int mode, String outDir);
}
