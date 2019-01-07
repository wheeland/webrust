<?php

$msg = $_GET['msg'];

$descriptors = array(
    0 => array("pipe", "r"),  // STDIN
    1 => array("pipe", "w"),  // STDOUT
    2 => array("pipe", "w")   // STDERR
);

$proc = proc_open("./tetris-server", $descriptors, $pipes);
fwrite($pipes[0], "$msg");
fclose($pipes[0]);

$stdout = stream_get_contents($pipes[1]);
$stderr = stream_get_contents($pipes[2]);

fclose($pipes[1]);
fclose($pipes[2]);

$exitCode = proc_close($proc);

header("Content-Type: application/octet-stream");
echo $stdout;
?>
