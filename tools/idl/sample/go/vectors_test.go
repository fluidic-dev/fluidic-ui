package sample

import (
	"bufio"
	"fmt"
	"os"
	"strings"
	"testing"
)

// vector runs one line of ../vectors.txt for one type.
type vector func(fields []string) error

func typed[T any](decode func([]byte) (T, error), encode func(T) ([]byte, error), unknown func(T) (string, bool)) vector {
	return func(fields []string) error {
		kind, text := fields[0], fields[2]
		v, err := decode([]byte(text))
		switch kind {
		case "refuse":
			if err == nil {
				return fmt.Errorf("decoded to %#v", v)
			}
			return nil
		case "round", "normal":
			if err != nil {
				return fmt.Errorf("not decoded: %v", err)
			}
			want := text
			if kind == "normal" {
				want = fields[3]
			}
			got, err := encode(v)
			if err != nil {
				return fmt.Errorf("not encoded: %v", err)
			}
			if string(got) != want {
				return fmt.Errorf("encoded as %s", got)
			}
			return nil
		case "unknown", "noencode":
			if err != nil {
				return fmt.Errorf("not decoded: %v", err)
			}
			if kind == "unknown" {
				name, ok := unknown(v)
				if !ok || name != fields[3] {
					return fmt.Errorf("decoded to %#v", v)
				}
			}
			if got, err := encode(v); err == nil {
				return fmt.Errorf("encoded as %s", got)
			}
			return nil
		}
		return fmt.Errorf("no such kind %q", kind)
	}
}

func none[T any](T) (string, bool) { return "", false }

func scalar[T any](read func(*jsonReader) (T, error), write func(*jsonWriter, T) error) vector {
	return typed(
		func(b []byte) (T, error) { return decodeValue(b, read) },
		func(v T) ([]byte, error) { return encodeValue(v, write) },
		none[T],
	)
}

var vectors = map[string]vector{
	"sample":   typed(DecodeSample, EncodeSample, none[Sample]),
	"sdk_info": typed(DecodeSDKInfo, EncodeSDKInfo, none[SDKInfo]),
	"nothing":  typed(DecodeNothing, EncodeNothing, none[Nothing]),
	"node":     typed(DecodeNode, EncodeNode, none[Node]),
	"choice":   typed(DecodeChoice, EncodeChoice, none[Choice]),
	"tally":    typed(DecodeTally, EncodeTally, none[Tally]),
	"shape":    typed(DecodeShape, EncodeShape, none[Shape]),
	"group":    typed(DecodeGroup, EncodeGroup, none[Group]),
	"keywords": typed(DecodeKeywords, EncodeKeywords, none[Keywords]),
	"bye_reason": typed(DecodeByeReason, EncodeByeReason, func(v ByeReason) (string, bool) {
		u, ok := v.(ByeReasonUnknown)
		return u.Name, ok
	}),
	"alignment": typed(DecodeAlignment, EncodeAlignment, func(v Alignment) (string, bool) {
		return string(v), !v.Known()
	}),
	"app_message": typed(DecodeAppMessage, EncodeAppMessage, func(v AppMessage) (string, bool) {
		u, ok := v.(AppMessageUnknown)
		return u.Name, ok
	}),
	"host_message": typed(DecodeHostMessage, EncodeHostMessage, func(v HostMessage) (string, bool) {
		u, ok := v.(HostMessageUnknown)
		return u.Name, ok
	}),
	"bool":    scalar(readBool, writeBool),
	"u32":     scalar(readU32, writeU32),
	"i32":     scalar(readI32, writeI32),
	"u64":     scalar(readU64, writeU64),
	"i64":     scalar(readI64, writeI64),
	"f32":     scalar(readF32, writeF32),
	"f64":     scalar(readF64, writeF64),
	"string":  scalar(readString, writeString),
	"version": scalar(readVersion, writeVersion),
}

func TestSharedVectors(t *testing.T) {
	f, err := os.Open("../vectors.txt")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	scanner := bufio.NewScanner(f)
	n, count := 0, 0
	for scanner.Scan() {
		n++
		line := scanner.Text()
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		count++
		fields := strings.Split(line, "\t")
		run, ok := vectors[fields[1]]
		if !ok {
			t.Errorf("vectors.txt:%d: no such type %q", n, fields[1])
			continue
		}
		if err := run(fields); err != nil {
			t.Errorf("vectors.txt:%d: %s\n    %v", n, line, err)
		}
	}
	if err := scanner.Err(); err != nil {
		t.Fatal(err)
	}
	if count == 0 {
		t.Fatal("no vectors")
	}
}
