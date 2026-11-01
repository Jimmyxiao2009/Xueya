Add-Type -AssemblyName System.Speech
$taskNarrator = New-Object System.Speech.Synthesis.SpeechSynthesizer
$taskNarrator.SelectVoice('Microsoft Huihui Desktop')
$taskNarrator.Rate = 2
$taskSegments = Get-Content -LiteralPath 'scenes/competition/script.json' -Raw -Encoding UTF8 | ConvertFrom-Json
for ($taskIndex = 0; $taskIndex -lt $taskSegments.Count; $taskIndex++) {
  $taskPhrases = [regex]::Matches($taskSegments[$taskIndex].voice, '[^。！？]+[。！？]?')
  for ($taskPhrase = 0; $taskPhrase -lt $taskPhrases.Count; $taskPhrase++) {
    $taskAudioPath = Join-Path (Get-Location) ('audio/competition/phrase-{0}-{1}.wav' -f $taskIndex,$taskPhrase)
    $taskNarrator.SetOutputToWaveFile($taskAudioPath)
    $taskNarrator.Speak($taskPhrases[$taskPhrase].Value)
    $taskNarrator.SetOutputToNull()
  }
}
$taskNarrator.Dispose()
