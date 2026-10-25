# Spec Delta

## Purpose

Emular el procesador de sonido (SPC700, DSP y puertos de comunicación) para reproducir la música y los efectos del juego, con la subida real de programas y datos que hace el juego.

## ADDED Requirements

### Requirement: Puertos de comunicación
Los registros `$2140-$2143` SHALL comunicar la CPU principal con la SPC700 mediante cuatro puertos de entrada y cuatro de salida independientes, y tras el reinicio MUST dar `$AA` y `$BB` en los puertos 0 y 1 hasta que el juego inicie la subida.

#### Scenario: Arranque
- **WHEN** el juego lee `$2140` y `$2141` tras el reinicio
- **THEN** obtiene `$AA` y `$BB`

#### Scenario: Subida de un bloque
- **WHEN** el juego sube un bloque con el protocolo de la ROM de arranque (dirección, `$CC`, bytes numerados)
- **THEN** la SPC700 los guarda en su RAM y devuelve cada contador en el puerto 0

### Requirement: CPU SPC700
El emulador SHALL ejecutar el juego de instrucciones completo de la SPC700 con sus tres temporizadores, la ROM de arranque de 64 bytes y la RAM de 64 KiB, y MUST avanzar sincronizado con la CPU principal a razón de 1,024 MHz frente a los 1,79 MHz de esta.

#### Scenario: Programa de prueba
- **WHEN** se carga un programa que suma con `ADC` y escribe el resultado en un puerto
- **THEN** el puerto muestra el valor correcto y los indicadores quedan como en el hardware

### Requirement: DSP
El emulador SHALL implementar las ocho voces con muestras BRR, envolventes ADSR y GAIN, pitch, ruido, modulación de pitch, eco con filtro FIR y mezcla estéreo a 32 kHz.

#### Scenario: Voz sencilla
- **WHEN** una voz reproduce una muestra BRR en bucle con envolvente fija
- **THEN** la salida es una onda con el periodo que dicta el pitch

### Requirement: Salida de audio
El ejecutable interactivo SHALL enviar el audio a SDL a 32 kHz estéreo con un búfer que evite cortes, y MUST no dejar que el audio se adelante ni se retrase respecto a la imagen más de 100 ms.

#### Scenario: Pausa
- **WHEN** se pausa con P
- **THEN** el audio se silencia y se reanuda sin saltos

### Requirement: Salida a fichero
Con `--wav FICHERO` el ejecutable SHALL escribir la salida del audio de la ejecución como WAV estéreo de 16 bits a 32 kHz, para poder comprobarla sin dispositivo de sonido.

#### Scenario: Música del título
- **WHEN** se ejecutan 1200 fotogramas con `--no-window --wav w.wav`
- **THEN** el fichero contiene audio no silencioso de unos 20 segundos
