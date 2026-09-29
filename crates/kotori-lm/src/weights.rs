//! スコア統合の重み(docs/SPEC.md 6.2)。
//!
//! 候補 c の最終スコアは
//! S(c) = λ_lm · log P_LM(c) − λ_lattice · cost_lattice(c) / T + λ_user · bonus_user(c)。
//! 重みはモデルの GGUF のメタデータ `kotori.score_weights` に JSON で持つ(6.4)。

use serde::{Deserialize, Serialize};

use crate::{LmError, Model};

/// スコア統合の重み。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreWeights {
    pub lambda_lm: f32,
    pub lambda_lattice: f32,
    /// ラティスのコストを対数確率の尺度に合わせる温度 T。
    pub temperature: f32,
    pub lambda_user: f32,
}

impl ScoreWeights {
    /// JSON から読む。温度が正でなければ拒否する。
    pub fn from_json(json: &str) -> Result<Self, LmError> {
        let w: Self = serde_json::from_str(json).map_err(|_| LmError::Weights)?;
        let finite = [w.lambda_lm, w.lambda_lattice, w.temperature, w.lambda_user]
            .iter()
            .all(|x| x.is_finite());
        if finite && w.temperature > 0.0 {
            Ok(w)
        } else {
            Err(LmError::Weights)
        }
    }

    /// モデルのメタデータ `kotori.score_weights` から読む。
    pub fn from_model(model: &Model) -> Result<Self, LmError> {
        Self::from_json(&model.meta("kotori.score_weights").ok_or(LmError::Weights)?)
    }

    /// 最終スコア S(c)。
    pub fn combine(&self, lm_logp: f32, lattice_cost: i64, user_bonus: f32) -> f32 {
        self.lambda_lm * lm_logp - self.lambda_lattice * lattice_cost as f32 / self.temperature
            + self.lambda_user * user_bonus
    }
}
