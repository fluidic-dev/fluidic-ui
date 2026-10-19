package sample

import (
	"math"
	"testing"
)

func TestNaNAndInfinityAreNotEncoded(t *testing.T) {
	for _, v := range []float64{math.NaN(), math.Inf(1), math.Inf(-1)} {
		if got, err := encodeValue(v, writeF64); err == nil {
			t.Errorf("%v was encoded as %s", v, got)
		}
		if got, err := encodeValue(float32(v), writeF32); err == nil {
			t.Errorf("%v was encoded as an f32, %s", v, got)
		}
	}
}

func TestAValueHoldingNaNIsNotEncoded(t *testing.T) {
	v := Sample{Reason: ByeReasonDone{}, Align: AlignmentStart, Scale: math.NaN()}
	if got, err := EncodeSample(v); err == nil {
		t.Errorf("encoded as %s", got)
	}
}
